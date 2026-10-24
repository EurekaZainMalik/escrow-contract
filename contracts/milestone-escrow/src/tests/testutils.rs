//! Shared escrow fixtures for unit tests (#743).
//!
//! Test files used to build their own escrows -- register a token, mint,
//! `initialize`, `fund` -- each in a slightly different way, so every change
//! to the `initialize` signature meant hand-editing dozens of helpers.
//! `EscrowFixture` is the one place that knows how to do that:
//!
//! ```ignore
//! let fx = EscrowFixture::new(&env).milestones(&[100, 200]).funded().build();
//! fx.escrow.mark_delivered(&fx.freelancer, &0);
//! ```
//!
//! Auth is mocked explicitly, per setup call, through the client-level
//! `mock_auths`. The SDK restores the previous auth manager after each such
//! call, so the fixture neither needs nor changes whatever auth mode the
//! test itself chose (`mock_all_auths`, targeted `mock_auths`, or none).

use crate::{MilestoneEscrow, MilestoneEscrowClient};
use soroban_sdk::testutils::{Address as _, MockAuth, MockAuthInvoke};
use soroban_sdk::{token, Address, Env, IntoVal, Vec};

/// Auto-release window used when a test does not pick one (seven days).
pub(crate) const DEFAULT_AUTO_RELEASE_SECONDS: u64 = 604_800;

/// How far the builder takes the escrow before handing it over.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Stage {
    /// Contract registered, `initialize` never called.
    Registered,
    /// `initialize` committed; nothing minted or funded.
    Initialized,
    /// Milestone total minted to the client and deposited via `fund`.
    Funded,
}

/// Builder for a `MilestoneEscrow` under test.
///
/// Defaults: a single 1 000-unit milestone, a seven-day auto-release window,
/// and an initialized (but unfunded) escrow.
pub(crate) struct EscrowFixture<'a> {
    env: &'a Env,
    milestones: std::vec::Vec<i128>,
    auto_release_seconds: u64,
    stage: Stage,
}

/// Handles to a built escrow and the parties recorded on it.
pub(crate) struct Escrow<'a> {
    /// Client for calling the escrow contract.
    pub escrow: MilestoneEscrowClient<'a>,
    pub contract_id: Address,
    pub client: Address,
    pub freelancer: Address,
    pub arbiter: Address,
    pub admin: Address,
    /// Stellar asset contract used as the escrow token; `admin` is its issuer.
    pub token: Address,
}

impl<'a> EscrowFixture<'a> {
    pub fn new(env: &'a Env) -> Self {
        Self {
            env,
            milestones: std::vec![1_000],
            auto_release_seconds: DEFAULT_AUTO_RELEASE_SECONDS,
            stage: Stage::Initialized,
        }
    }

    pub fn milestones(mut self, amounts: &[i128]) -> Self {
        self.milestones = amounts.to_vec();
        self
    }

    pub fn auto_release_seconds(mut self, seconds: u64) -> Self {
        self.auto_release_seconds = seconds;
        self
    }

    /// Stop after registering the contract; `initialize` is never called.
    pub fn uninitialized(mut self) -> Self {
        self.stage = Stage::Registered;
        self
    }

    /// Mint the milestone total to the client and `fund` the escrow.
    pub fn funded(mut self) -> Self {
        self.stage = Stage::Funded;
        self
    }

    pub fn build(self) -> Escrow<'a> {
        let env = self.env;
        // Generation order matches the historical `setup_funded_escrow`, so
        // migrated tests see the same addresses as before.
        let client = Address::generate(env);
        let freelancer = Address::generate(env);
        let arbiter = Address::generate(env);
        let admin = Address::generate(env);

        let token = env
            .register_stellar_asset_contract_v2(admin.clone())
            .address();
        let amounts = Vec::from_slice(env, &self.milestones);
        let spec_hashes = crate::spec_hashes_for(&amounts);
        let total: i128 = self.milestones.iter().sum();

        if self.stage >= Stage::Funded {
            let mint_args = (&client, &total).into_val(env);
            token::StellarAssetClient::new(env, &token)
                .mock_auths(&[MockAuth {
                    address: &admin,
                    invoke: &MockAuthInvoke {
                        contract: &token,
                        fn_name: "mint",
                        args: mint_args,
                        sub_invokes: &[],
                    },
                }])
                .mint(&client, &total);
        }

        let contract_id = env.register(MilestoneEscrow, ());
        let escrow = MilestoneEscrowClient::new(env, &contract_id);

        if self.stage >= Stage::Initialized {
            let init_args = (
                &admin,
                &client,
                &freelancer,
                &arbiter,
                &token,
                &self.auto_release_seconds,
                &amounts,
                &spec_hashes,
            )
                .into_val(env);
            escrow
                .mock_auths(&[MockAuth {
                    address: &admin,
                    invoke: &MockAuthInvoke {
                        contract: &contract_id,
                        fn_name: "initialize",
                        args: init_args,
                        sub_invokes: &[],
                    },
                }])
                .initialize(
                    &admin,
                    &client,
                    &freelancer,
                    &arbiter,
                    &token,
                    &self.auto_release_seconds,
                    &amounts,
                    &spec_hashes,
                );
            Self::accept(env, &escrow, &contract_id, "accept_job", &freelancer);
            Self::accept(env, &escrow, &contract_id, "accept_arbiter", &arbiter);
        }

        if self.stage >= Stage::Funded {
            let deposit_args = (&client, &contract_id, &total).into_val(env);
            let fund_args = (&client,).into_val(env);
            escrow
                .mock_auths(&[MockAuth {
                    address: &client,
                    invoke: &MockAuthInvoke {
                        contract: &contract_id,
                        fn_name: "fund",
                        args: fund_args,
                        sub_invokes: &[MockAuthInvoke {
                            contract: &token,
                            fn_name: "transfer",
                            args: deposit_args,
                            sub_invokes: &[],
                        }],
                    },
                }])
                .fund(&client);
        }

        Escrow {
            escrow,
            contract_id,
            client,
            freelancer,
            arbiter,
            admin,
            token,
        }
    }
}

impl EscrowFixture<'_> {
    /// Record `party`'s acceptance via `fn_name` (`accept_job` or
    /// `accept_arbiter`), mocking only that party's auth.
    fn accept(
        env: &Env,
        escrow: &MilestoneEscrowClient<'_>,
        contract_id: &Address,
        fn_name: &str,
        party: &Address,
    ) {
        let args = (party,).into_val(env);
        let invoke = MockAuthInvoke {
            contract: contract_id,
            fn_name,
            args,
            sub_invokes: &[],
        };
        let auths = [MockAuth {
            address: party,
            invoke: &invoke,
        }];
        let escrow = escrow.mock_auths(&auths);
        if fn_name == "accept_job" {
            escrow.accept_job(party);
        } else {
            escrow.accept_arbiter(party);
        }
    }
}

impl<'a> Escrow<'a> {
    /// Token client for balance checks against the escrow token.
    pub fn token_client(&self) -> token::Client<'a> {
        token::Client::new(&self.escrow.env, &self.token)
    }
}

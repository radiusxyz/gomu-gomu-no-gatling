use crate::config::{CreateAccounts, GatlingConfig};
use color_eyre::eyre::Result;
use lazy_static::lazy_static;
use log::info;

use starknet::accounts::{Account, AccountError, SingleOwnerAccount};
use starknet::core::chain_id;
use starknet::core::types::{
    contract::legacy::LegacyContractClass, BlockId, BlockTag, FieldElement, StarknetError,
};
use starknet::macros::felt;
use starknet::providers::{jsonrpc::HttpTransport, JsonRpcClient, Provider, ProviderError};
use starknet::signers::{LocalWallet, SigningKey};
use std::str;
use std::sync::Arc;
use url::Url;

lazy_static! {
    pub static ref FEE_TOKEN_ADDRESS: FieldElement =
        felt!("0x49d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7");
    pub static ref OZ_CLASS_HASH: FieldElement =
        felt!("0x045e3ac97b1c575540dbf6b6f089f390f00fa98928415bb91a27a43790b52f30");
}

/// Shoot the load test simulation.
pub async fn shoot(config: GatlingConfig) -> Result<SimulationReport> {
    info!("starting simulation with config: {:?}", config);
    let mut shooter = GatlingShooter::new(config)?;
    let mut simulation_report = Default::default();
    // Trigger the setup phase.
    shooter.setup(&mut simulation_report).await?;
    // Run the simulation.
    shooter.run(&mut simulation_report).await?;
    // Trigger the teardown phase.
    shooter.teardown(&mut simulation_report).await?;
    Ok(simulation_report.clone())
}

pub struct GatlingShooter {
    config: GatlingConfig,
    starknet_rpc: JsonRpcClient<HttpTransport>,
    signer: LocalWallet,
    address: FieldElement,
}

impl GatlingShooter {
    pub fn new(config: GatlingConfig) -> Result<Self> {
        let starknet_rpc =
            starknet_rpc_provider(Url::parse(&config.clone().rpc.unwrap_or_default().url)?);
        let deployer = config.clone().deployer.unwrap_or_default();

        let signer = LocalWallet::from(SigningKey::from_secret_scalar(
            FieldElement::from_hex_be(deployer.signing_key.as_str()).unwrap_or_default(),
        ));

        // implement let account = Arc::new(account); instead of signer
        let address = FieldElement::from_hex_be(deployer.address.as_str()).unwrap_or_default();

        Ok(Self {
            config,
            starknet_rpc,
            signer,
            address,
        })
    }

    /// Setup the simulation.
    async fn setup<'a>(&mut self, _simulation_report: &'a mut SimulationReport) -> Result<()> {
        let chain_id = self.starknet_rpc.chain_id().await?.to_bytes_be();
        let block_number = self.starknet_rpc.block_number().await?;
        println!(
            "Shoot - {} @ block number - {}",
            str::from_utf8(&chain_id)?.trim_start_matches('\0'),
            block_number
        );

        self.declare_contract_legacy("contracts/v0/ERC20.json")
            .await;

        self.declare_contract_legacy("contracts/v0/ERC721.json")
            .await;

        if let Some(setup) = self.config.clone().simulation.unwrap_or_default().setup {
            if let Some(create_accounts) = setup.create_accounts {
                self.declare_contract_legacy("contracts/v0/OpenzeppelinAccount.json")
                    .await;

                self.create_accounts(_simulation_report, create_accounts)
                    .await?;
            }
        }

        Ok(())
    }

    /// Teardown the simulation.
    async fn teardown<'a>(&mut self, _simulation_report: &'a mut SimulationReport) -> Result<()> {
        info!("tearing down!");
        Ok(())
    }

    /// Run the simulation.
    async fn run<'a>(&mut self, _simulation_report: &'a mut SimulationReport) -> Result<()> {
        info!("firing!");
        let _fail_fast = self.config.simulation.clone().unwrap_or_default().fail_fast;
        Ok(())
    }

    /// Create accounts.
    async fn create_accounts<'a>(
        &mut self,
        _simulation_report: &'a mut SimulationReport,
        account_details: CreateAccounts,
    ) -> Result<()> {
        println!("\tcreating {} accounts", account_details.num_accounts);

        for i in 0..account_details.num_accounts {
            // let mut account = SingleOwnerAccount::new(
            //     &self.starknet_rpc,
            //     self.signer.clone(),
            //     self.address,
            //     chain_id::TESTNET,
            // );

            // account.set_block_id(BlockId::Tag(BlockTag::Pending));

            // let contract_factory = ContractFactory::new(*OZ_CLASS_HASH, account);
            // contract_factory
            //     .deploy(&vec![felt!("123")], felt!("456"), false)
            //     .send()
            //     .await
            //     .expect("Unable to deploy contract");

            // TODO: fund accounts and add timing

            print!(".{i}");
        }

        Ok(())
    }

    async fn declare_contract_legacy<'a>(&self, contract_path: &str) {
        let contract_artifact: LegacyContractClass =
            serde_json::from_reader(std::fs::File::open(contract_path).unwrap()).unwrap();

        let mut account = SingleOwnerAccount::new(
            &self.starknet_rpc,
            self.signer.clone(),
            self.address,
            chain_id::TESTNET,
        );

        account.set_block_id(BlockId::Tag(BlockTag::Pending));

        match account
            .declare_legacy(Arc::new(contract_artifact))
            .send()
            .await
        {
            Ok(tx_resp) => {
                info!("Declared Contract TX: {:?}", tx_resp.transaction_hash);
            }
            Err(AccountError::Provider(ProviderError::StarknetError(
                StarknetError::ClassAlreadyDeclared,
            ))) => {
                info!("Contract already declared");
            }
            Err(e) => {
                panic!("could not declare contract: {e}");
            }
        };
    }
}

/// The simulation report.
#[derive(Debug, Default, Clone)]
pub struct SimulationReport {
    pub chain_id: Option<FieldElement>,
    pub block_number: Option<u64>,
}

/// Create a StarkNet RPC provider from a URL.
/// # Arguments
/// * `rpc` - The URL of the StarkNet RPC provider.
/// # Returns
/// A StarkNet RPC provider.
fn starknet_rpc_provider(rpc: Url) -> JsonRpcClient<HttpTransport> {
    JsonRpcClient::new(HttpTransport::new(rpc))
}

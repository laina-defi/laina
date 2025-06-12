pub const WASM: &[u8] = soroban_sdk::contractfile!(
    file = "../target/wasm32v1-none/release/reflector_oracle.wasm", sha256 =
    "df88820e231ad8f3027871e5dd3cf45491d7b7735e785731466bfc2946008608"
);
#[soroban_sdk::contractargs(name = "Args")]
#[soroban_sdk::contractclient(name = "Client")]
pub trait Contract {
    fn base(env: soroban_sdk::Env) -> Asset;
    fn decimals(env: soroban_sdk::Env) -> u32;
    fn resolution(env: soroban_sdk::Env) -> u32;
    fn period(env: soroban_sdk::Env) -> Option<u64>;
    fn assets(env: soroban_sdk::Env) -> soroban_sdk::Vec<Asset>;
    fn last_timestamp(env: soroban_sdk::Env) -> u64;
    fn price(env: soroban_sdk::Env, asset: Asset, timestamp: u64) -> Option<PriceData>;
    fn lastprice(env: soroban_sdk::Env, asset: Asset) -> Option<PriceData>;
    fn prices(
        env: soroban_sdk::Env,
        asset: Asset,
        records: u32,
    ) -> Option<soroban_sdk::Vec<PriceData>>;
    fn x_last_price(
        env: soroban_sdk::Env,
        base_asset: Asset,
        quote_asset: Asset,
    ) -> Option<PriceData>;
    fn x_price(
        env: soroban_sdk::Env,
        base_asset: Asset,
        quote_asset: Asset,
        timestamp: u64,
    ) -> Option<PriceData>;
    fn x_prices(
        env: soroban_sdk::Env,
        base_asset: Asset,
        quote_asset: Asset,
        records: u32,
    ) -> Option<soroban_sdk::Vec<PriceData>>;
    fn twap(env: soroban_sdk::Env, asset: Asset, records: u32) -> Option<i128>;
    fn x_twap(
        env: soroban_sdk::Env,
        base_asset: Asset,
        quote_asset: Asset,
        records: u32,
    ) -> Option<i128>;
    fn version(env: soroban_sdk::Env) -> u32;
    fn admin(env: soroban_sdk::Env) -> Option<soroban_sdk::Address>;
    fn config(env: soroban_sdk::Env, config: ConfigData);
    fn add_assets(env: soroban_sdk::Env, assets: soroban_sdk::Vec<Asset>);
    fn set_period(env: soroban_sdk::Env, period: u64);
    fn set_price(env: soroban_sdk::Env, updates: soroban_sdk::Vec<i128>, timestamp: u64);
    fn update_contract(env: soroban_sdk::Env, wasm_hash: soroban_sdk::BytesN<32>);
}
#[soroban_sdk::contracttype(export = false)]
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct ConfigData {
    pub admin: soroban_sdk::Address,
    pub assets: soroban_sdk::Vec<Asset>,
    pub base_asset: Asset,
    pub decimals: u32,
    pub period: u64,
    pub resolution: u32,
}
#[soroban_sdk::contracttype(export = false)]
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct PriceData {
    pub price: i128,
    pub timestamp: u64,
}
#[soroban_sdk::contracttype(export = false)]
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum Asset {
    Stellar(soroban_sdk::Address),
    Other(soroban_sdk::Symbol),
}
#[soroban_sdk::contracterror(export = false)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum Error {
    AlreadyInitialized = 0,
    Unauthorized = 1,
    AssetMissing = 2,
    AssetAlreadyExists = 3,
    InvalidConfigVersion = 4,
    InvalidTimestamp = 5,
    InvalidUpdateLength = 6,
    AssetLimitExceeded = 7,
}


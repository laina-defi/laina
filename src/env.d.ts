/// <reference path="../.astro/types.d.ts" />
/// <reference types="astro/client" />

interface ImportMetaEnv {
  readonly SOROBAN_NETWORK_PASSPHRASE: string;
  readonly SOROBAN_RPC_URL: string;
  readonly SOROBAN_SOURCE_ACCOUNT: string;
  readonly CONTRACT_ADDRESS_XLM: string;
  readonly CONTRACT_ADDRESS_USDC: string;
  readonly CONTRACT_ADDRESS_EURC: string;
  readonly ISSUER_ADDRESS_USDC: string;
  readonly ISSUER_ADDRESS_EURC: string;
}

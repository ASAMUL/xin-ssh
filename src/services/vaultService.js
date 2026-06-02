import { Stronghold } from "@tauri-apps/plugin-stronghold";

const CLIENT_NAME = "xin-terminal";
const encoder = new TextEncoder();
const decoder = new TextDecoder();

let stronghold = null;
let client = null;
let store = null;

export async function unlockStronghold(vaultPath, unlockPassword, onProgress) {
  stronghold = await runVaultStep(
    "loadStronghold",
    () => Stronghold.load(vaultPath, unlockPassword),
    onProgress,
  );

  try {
    client = await runVaultStep("loadClient", () => stronghold.loadClient(CLIENT_NAME), onProgress);
  } catch {
    client = await runVaultStep("createClient", () => stronghold.createClient(CLIENT_NAME), onProgress);
  }

  store = client.getStore();
}

export async function lockStronghold(onProgress) {
  if (stronghold) {
    await runVaultStep("unloadStronghold", () => stronghold.unload(), onProgress);
  }

  stronghold = null;
  client = null;
  store = null;
}

export async function saveCredential(credentialRef, secret, onProgress) {
  assertUnlocked();

  if (secret.password) {
    await runVaultStep(
      "insertPassword",
      () => store.insert(secretKey(credentialRef, "password"), bytes(secret.password)),
      onProgress,
    );
  }

  if (secret.passphrase) {
    await runVaultStep(
      "insertPassphrase",
      () => store.insert(secretKey(credentialRef, "passphrase"), bytes(secret.passphrase)),
      onProgress,
    );
  }

  await runVaultStep("saveStronghold", () => stronghold.save(), onProgress);
}

export async function readCredential(credentialRef, kind) {
  assertUnlocked();
  const value = await store.get(secretKey(credentialRef, kind));
  return value ? decoder.decode(value) : null;
}

function assertUnlocked() {
  if (!store || !stronghold) {
    throw new Error("Credential store is not loaded");
  }
}

function secretKey(credentialRef, kind) {
  return `credential:${credentialRef}:${kind}`;
}

function bytes(value) {
  return Array.from(encoder.encode(value));
}

async function runVaultStep(step, task, onProgress) {
  onProgress?.(step);
  const startedAt = performance.now();

  try {
    return await task();
  } finally {
    console.info("[Xin Terminal] vault step", {
      step,
      durationMs: Math.round(performance.now() - startedAt),
    });
  }
}

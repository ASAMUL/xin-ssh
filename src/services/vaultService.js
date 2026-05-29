import { Stronghold } from "@tauri-apps/plugin-stronghold";

const CLIENT_NAME = "xin-terminal";
const MARKER_KEY = "vault:marker";
const encoder = new TextEncoder();
const decoder = new TextDecoder();

let stronghold = null;
let client = null;
let store = null;

export async function unlockStronghold(vaultPath, unlockPassword) {
  stronghold = await Stronghold.load(vaultPath, unlockPassword);

  try {
    client = await stronghold.loadClient(CLIENT_NAME);
  } catch {
    client = await stronghold.createClient(CLIENT_NAME);
  }

  store = client.getStore();
  const marker = await store.get(MARKER_KEY);
  if (!marker) {
    await store.insert(MARKER_KEY, bytes("xin-terminal-vault"));
    await stronghold.save();
  }
}

export async function lockStronghold() {
  if (stronghold) {
    await stronghold.unload();
  }

  stronghold = null;
  client = null;
  store = null;
}

export async function saveCredential(credentialRef, secret) {
  assertUnlocked();

  if (secret.password) {
    await store.insert(secretKey(credentialRef, "password"), bytes(secret.password));
  }

  if (secret.passphrase) {
    await store.insert(secretKey(credentialRef, "passphrase"), bytes(secret.passphrase));
  }

  await stronghold.save();
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

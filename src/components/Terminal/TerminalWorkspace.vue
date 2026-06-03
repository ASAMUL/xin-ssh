<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import { listen } from "@tauri-apps/api/event";
import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import { useI18n } from "vue-i18n";
import FileExplorer from "../FileExplorer.vue";
import { useSessionStore } from "../../stores/sessionStore";

const TERMINAL_EVENT_NAME = "terminal-event";
const { t } = useI18n();
const sessionStore = useSessionStore();
const terminalMount = ref(null);
const credentialInput = ref(null);
const credentials = reactive({
  password: "",
  passphrase: "",
});
let terminal = null;
let fitAddon = null;
let resizeObserver = null;
let unlistenTerminal = null;
let disposeDataListener = null;
let disposeResizeListener = null;
let lastWrittenSessionId = null;
let lastWrittenLength = 0;

const activeSession = computed(() => sessionStore.activeSession);
const isPasswordAuth = computed(() => activeSession.value?.authMethod === "password");
const isPrivateKeyAuth = computed(() => activeSession.value?.authMethod === "privateKey");
const needsCredentials = computed(() => activeSession.value?.status === "authRequired");
const terminalText = computed(() => activeSession.value?.terminalOutput || "");
const terminalStatusText = computed(() => {
  const session = activeSession.value;
  if (!session) return "";

  return t(`monitor.status.${session.status}`);
});

watch(
  () => activeSession.value?.id,
  async () => {
    credentials.password = "";
    credentials.passphrase = "";
    await nextTick();
    syncTerminalBuffer(true);
    fitTerminal();
    focusActiveInput();
  },
);

watch(
  () => terminalText.value.length,
  () => {
    syncTerminalBuffer(false);
  },
);

watch(needsCredentials, async () => {
  await nextTick();
  focusActiveInput();
});

onMounted(async () => {
  terminal = new Terminal({
    allowProposedApi: false,
    cursorBlink: true,
    cursorStyle: "block",
    fontFamily: '"Cascadia Code", "SFMono-Regular", Consolas, monospace',
    fontSize: 14,
    lineHeight: 1.18,
    scrollback: 8000,
    theme: {
      background: "#0d1c29",
      foreground: "#eef6ff",
      cursor: "#30e38c",
      selectionBackground: "#2e5d7a",
      black: "#0b1723",
      blue: "#4aa3ff",
      cyan: "#33d1c5",
      green: "#35d486",
      magenta: "#c084fc",
      red: "#f87171",
      white: "#e5eef8",
      yellow: "#facc15",
      brightBlack: "#64748b",
      brightBlue: "#7cc6ff",
      brightCyan: "#67e8f9",
      brightGreen: "#86efac",
      brightMagenta: "#d8b4fe",
      brightRed: "#fca5a5",
      brightWhite: "#ffffff",
      brightYellow: "#fde68a",
    },
  });
  fitAddon = new FitAddon();
  terminal.loadAddon(fitAddon);
  terminal.open(terminalMount.value);

  disposeDataListener = terminal.onData((data) => {
    sessionStore.sendActiveTerminalInput(data).catch(() => {});
  });
  disposeResizeListener = terminal.onResize(({ cols, rows }) => {
    sessionStore.resizeActiveTerminal(cols, rows);
  });

  resizeObserver = new ResizeObserver(() => {
    fitTerminal();
  });
  resizeObserver.observe(terminalMount.value);

  unlistenTerminal = await listen(TERMINAL_EVENT_NAME, (event) => {
    sessionStore.handleTerminalEvent(event.payload);
  });

  await nextTick();
  syncTerminalBuffer(true);
  fitTerminal();
  focusActiveInput();
});

onBeforeUnmount(() => {
  unlistenTerminal?.();
  resizeObserver?.disconnect();
  disposeDataListener?.dispose();
  disposeResizeListener?.dispose();
  terminal?.dispose();
});

function focusTerminal() {
  if (needsCredentials.value) return;

  terminal?.focus();
}

function focusCredentialInput() {
  credentialInput.value?.focus();
}

function focusActiveInput() {
  if (needsCredentials.value) {
    focusCredentialInput();
    return;
  }

  focusTerminal();
}

function fitTerminal() {
  if (!terminal || !fitAddon || !terminalMount.value) return;

  fitAddon.fit();
}

function syncTerminalBuffer(forceReset) {
  if (!terminal || !activeSession.value) return;

  const sessionId = activeSession.value.id;
  const output = terminalText.value;
  const sessionChanged = lastWrittenSessionId !== sessionId;
  const outputCleared = output.length < lastWrittenLength;

  if (forceReset || sessionChanged || outputCleared) {
    terminal.reset();
    terminal.write(output);
    lastWrittenSessionId = sessionId;
    lastWrittenLength = output.length;
    return;
  }

  if (output.length > lastWrittenLength) {
    terminal.write(output.slice(lastWrittenLength));
    lastWrittenLength = output.length;
  }
}

function startTerminal() {
  sessionStore.startActiveTerminal({
    password: credentials.password,
    passphrase: credentials.passphrase,
  });
  focusTerminal();
}

function reconnectTerminal() {
  credentials.password = "";
  credentials.passphrase = "";
  sessionStore.requestCredentials();
}
</script>

<template>
  <section class="terminal-workspace">
    <section class="terminal-panel-top">
      <header class="terminal-panel-toolbar">
        <div class="terminal-title-block">
          <span class="terminal-live-dot" :class="activeSession?.status"></span>
          <div>
            <strong>{{ activeSession?.name }}</strong>
            <small>{{ activeSession?.username }}@{{ activeSession?.host }}:{{ activeSession?.port }}</small>
          </div>
        </div>

        <div class="terminal-window-actions">
          <span class="terminal-state-label">{{ terminalStatusText }}</span>
          <button type="button" :aria-label="t('terminal.clear')" @click="sessionStore.clearActiveTerminal">
            <UIcon name="i-lucide-eraser" />
          </button>
          <button type="button" :aria-label="t('terminal.reconnect')" @click="reconnectTerminal">
            <UIcon name="i-lucide-rotate-cw" />
          </button>
        </div>
      </header>

      <div class="terminal-screen" @click="focusTerminal">
        <div ref="terminalMount" class="terminal-xterm"></div>

        <form v-if="needsCredentials" class="terminal-auth-panel" @click.stop @submit.prevent="startTerminal">
          <div class="auth-panel-copy">
            <strong>{{ t("terminal.authTitle") }}</strong>
            <small>{{ t("terminal.authDescription") }}</small>
          </div>

          <label v-if="isPasswordAuth">
            <span>{{ t("profile.password") }}</span>
            <input
              ref="credentialInput"
              v-model="credentials.password"
              type="password"
              autocomplete="current-password"
            />
          </label>

          <label v-if="isPrivateKeyAuth">
            <span>{{ t("profile.privateKeyPath") }}</span>
            <input :value="activeSession.privateKeyPath || ''" type="text" readonly />
          </label>

          <label v-if="isPrivateKeyAuth">
            <span>{{ t("profile.passphrase") }}</span>
            <input
              ref="credentialInput"
              v-model="credentials.passphrase"
              type="password"
              autocomplete="current-password"
            />
          </label>

          <p v-if="activeSession?.error" class="terminal-auth-error">{{ activeSession.error }}</p>

          <button type="submit" class="primary-action" :disabled="activeSession?.status === 'connecting'">
            <UIcon
              :name="activeSession?.status === 'connecting' ? 'i-lucide-loader-circle' : 'i-lucide-plug-zap'"
              :class="{ 'spin-icon': activeSession?.status === 'connecting' }"
            />
            <span>{{ t("terminal.connectReal") }}</span>
          </button>
        </form>

        <div v-else-if="activeSession?.status === 'connecting'" class="terminal-connecting">
          <UIcon name="i-lucide-loader-circle" class="spin-icon" />
          <span>{{ t("terminal.connecting") }}</span>
        </div>

        <div v-else-if="activeSession?.status === 'error'" class="terminal-connecting error">
          <UIcon name="i-lucide-circle-alert" />
          <span>{{ activeSession.error }}</span>
        </div>
      </div>
    </section>

    <FileExplorer :active-session="activeSession" />
  </section>
</template>

<style scoped src="../../assets/css/terminal-workspace.css"></style>

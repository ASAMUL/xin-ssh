<script setup>
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useTerminalGuardStore } from "../../stores/terminalGuardStore";

// 生产机保护的确认框（请求来自 useTerminalGuard）。
// 不放右上角的关闭叉：打开时焦点落在第一个可聚焦的元素上，这样它就是「取消」——
// 手快一路回车下去，结果是取消而不是执行。Esc 和点遮罩也都是取消。
const PREVIEW_LINES = 8;

const { t } = useI18n();
const guardStore = useTerminalGuardStore();
const request = computed(() => guardStore.request);
const isCommand = computed(() => request.value?.kind === "command");

const title = computed(() => (isCommand.value
  ? t("guard.commandTitle")
  : t("guard.pasteTitle", { count: request.value?.lines?.length ?? 0 })));
const description = computed(() => {
  const name = request.value?.sessionName ?? "";
  if (!isCommand.value) return t("guard.pasteDescription", { name });
  return t("guard.commandDescription", { name, reason: t(`guard.kinds.${request.value?.danger?.kind}`) });
});

// 命令拆成三截，命中危险规则的那一段标出来：长命令里一眼看到是哪儿触发的
const commandParts = computed(() => {
  const command = request.value?.command ?? "";
  const segment = request.value?.danger?.segment ?? "";
  const index = segment ? command.indexOf(segment) : -1;
  if (index < 0) return { before: command, match: "", after: "" };
  return {
    before: command.slice(0, index),
    match: segment,
    after: command.slice(index + segment.length),
  };
});
const previewLines = computed(() => request.value?.lines?.slice(0, PREVIEW_LINES) ?? []);
const hiddenLineCount = computed(() => Math.max(0, (request.value?.lines?.length ?? 0) - PREVIEW_LINES));
</script>

<template>
  <UModal
    :open="Boolean(request)"
    :title="title"
    :description="description"
    :close="false"
    :ui="{ footer: 'justify-end' }"
    @update:open="(open) => !open && guardStore.answer(false)"
  >
    <template #body>
      <pre v-if="isCommand" class="guard-code"><span>{{ commandParts.before }}</span><mark>{{ commandParts.match }}</mark><span>{{ commandParts.after }}</span></pre>
      <template v-else>
        <pre class="guard-code">{{ previewLines.join("\n") }}</pre>
        <p v-if="hiddenLineCount" class="guard-more">{{ t("guard.moreLines", { count: hiddenLineCount }) }}</p>
      </template>
    </template>
    <template #footer>
      <UButton color="neutral" variant="outline" :label="t('common.cancel')" @click="guardStore.answer(false)" />
      <UButton
        color="error"
        :icon="isCommand ? 'i-lucide-corner-down-left' : 'i-lucide-clipboard-paste'"
        :label="isCommand ? t('guard.run') : t('guard.paste')"
        @click="guardStore.answer(true)"
      />
    </template>
  </UModal>
</template>

<style scoped src="../../assets/css/terminal-guard-dialog.css"></style>

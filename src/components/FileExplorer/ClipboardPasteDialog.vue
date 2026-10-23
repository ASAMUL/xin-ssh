<script setup>
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { readClipboardImage } from "../../services/workspaceApi";
import { useFileExplorerStore } from "../../stores/fileExplorerStore";
import { ERROR_CODES, hasErrorCode, localizeError, toAppError } from "../../utils/appError";
import { countLines, decodeClipboardImage, defaultPasteFileName } from "../../utils/clipboardPaste";
import { formatBytes } from "../../utils/formatMetrics";
import { joinRemotePath } from "../../utils/remotePath";

// 预览只放开头这么多字：几兆的日志全塞进 DOM 会卡，保存的仍是完整内容
const TEXT_PREVIEW_CHARS = 4000;

const open = defineModel("open", { type: Boolean, default: false });
const props = defineProps({
  session: {
    type: Object,
    default: null,
  },
  directory: {
    type: String,
    default: "/",
  },
  // read_clipboard 的结果：{ text, hasImage }
  content: {
    type: Object,
    default: null,
  },
});

const { t } = useI18n();
const fileExplorerStore = useFileExplorerStore();
const textEncoder = new TextEncoder();
const mode = ref("text");
const fileName = ref("");
const nameInput = ref(null);
const image = ref(null);
const imageError = ref("");
const isLoadingImage = ref(false);
const unixLineEndings = ref(true);
const error = ref(null);
const isSubmitting = ref(false);

const text = computed(() => props.content?.text ?? "");
const modeItems = computed(() => [
  props.content?.hasImage && { label: t("paste.imageMode"), value: "image", icon: "i-lucide-image" },
  text.value && { label: t("paste.textMode"), value: "text", icon: "i-lucide-file-text" },
].filter(Boolean));
const hasCrlf = computed(() => text.value.includes("\r\n"));
const textPreview = computed(() => text.value.slice(0, TEXT_PREVIEW_CHARS));
const textMeta = computed(() => t("paste.textMeta", {
  lines: countLines(text.value),
  size: formatBytes(textEncoder.encode(text.value).length),
}));
const targetExists = computed(() => hasErrorCode(error.value, [ERROR_CODES.remoteTargetExists]));
const errorText = computed(() => {
  if (!error.value) return "";
  if (typeof error.value === "string") return error.value;
  return targetExists.value ? t("paste.targetExists") : localizeError(error.value);
});
const canSubmit = computed(() => mode.value === "text" || Boolean(image.value));

watch(open, (isOpen) => {
  if (!isOpen) {
    releaseImage();
    return;
  }
  unixLineEndings.value = true;
  switchMode(props.content?.hasImage ? "image" : "text");
});

onBeforeUnmount(releaseImage);

function switchMode(value) {
  mode.value = value;
  error.value = null;
  fileName.value = defaultPasteFileName(value, text.value);
  if (value === "image" && !image.value) loadImage();
  focusFileNameStem();
}

async function loadImage() {
  isLoadingImage.value = true;
  imageError.value = "";
  try {
    const decoded = await decodeClipboardImage(await readClipboardImage());
    if (!open.value) return;
    image.value = { ...decoded, url: URL.createObjectURL(decoded.blob) };
  } catch (loadError) {
    imageError.value = localizeError(loadError);
  } finally {
    isLoadingImage.value = false;
  }
}

function releaseImage() {
  if (image.value) URL.revokeObjectURL(image.value.url);
  image.value = null;
}

// 默认名里时间戳那段多半要改，选中它、留着扩展名，直接打字就能替换
async function focusFileNameStem() {
  await nextTick();
  const input = nameInput.value?.inputRef;
  if (!input) return;
  input.focus();
  const dot = fileName.value.lastIndexOf(".");
  input.setSelectionRange(0, dot > 0 ? dot : fileName.value.length);
}

async function submit(overwrite = false) {
  if (isSubmitting.value || !canSubmit.value || !props.session) return;

  const name = fileName.value;
  if (!name) {
    error.value = t("files.nameRequired");
    return;
  }
  if (name.includes("/") || name.includes("\0") || name === "." || name === "..") {
    error.value = t("files.nameCannotContainSlash");
    return;
  }

  isSubmitting.value = true;
  error.value = null;
  try {
    const bytes = mode.value === "image"
      ? new Uint8Array(await image.value.blob.arrayBuffer())
      : textEncoder.encode(unixLineEndings.value ? text.value.replaceAll("\r\n", "\n") : text.value);
    await fileExplorerStore.writeFileBytes(props.session, joinRemotePath(props.directory, name), bytes, overwrite);
    open.value = false;
  } catch (submitError) {
    error.value = toAppError(submitError);
  } finally {
    isSubmitting.value = false;
  }
}
</script>

<template>
  <UModal
    v-model:open="open"
    :title="t('paste.title')"
    :description="t('paste.description', { path: directory })"
    :dismissible="!isSubmitting"
    :close="!isSubmitting"
    :ui="{ content: 'paste-dialog', description: 'break-all', footer: 'justify-end' }"
    @after:enter="focusFileNameStem"
  >
    <template #body>
      <form class="paste-dialog-body" @submit.prevent="submit()">
        <UTabs
          v-if="modeItems.length > 1"
          :model-value="mode"
          class="paste-mode-switch"
          variant="pill"
          size="sm"
          :items="modeItems"
          :content="false"
          @update:model-value="switchMode"
        />

        <template v-if="mode === 'image'">
          <div class="paste-image-preview">
            <UIcon v-if="isLoadingImage" name="i-lucide-loader-circle" class="spin-icon paste-preview-spinner" />
            <p v-else-if="imageError" class="paste-preview-error">{{ imageError }}</p>
            <img v-else-if="image" :src="image.url" :alt="t('paste.imageAlt')" />
          </div>
          <p v-if="image" class="paste-meta">
            {{ image.width }} × {{ image.height }} · {{ formatBytes(image.blob.size) }}
          </p>
        </template>

        <template v-else>
          <pre class="paste-text-preview">{{ textPreview }}</pre>
          <p class="paste-meta">
            {{ textMeta }}
            <span v-if="text.length > textPreview.length"> · {{ t("paste.textTruncated") }}</span>
          </p>
          <UCheckbox
            v-if="hasCrlf"
            v-model="unixLineEndings"
            :label="t('paste.unixLineEndings')"
            :description="t('paste.unixLineEndingsHint')"
          />
        </template>

        <UFormField :label="t('paste.fileName')">
          <UInput
            ref="nameInput"
            v-model="fileName"
            class="w-full"
            color="neutral"
            variant="outline"
            spellcheck="false"
            @keydown.enter.prevent="submit()"
          />
        </UFormField>

        <UAlert
          v-if="errorText"
          :color="targetExists ? 'warning' : 'error'"
          variant="soft"
          :icon="targetExists ? 'i-lucide-copy-x' : 'i-lucide-circle-alert'"
          :title="errorText"
        />
      </form>
    </template>

    <template #footer>
      <UButton
        color="neutral"
        variant="outline"
        :label="t('common.cancel')"
        :disabled="isSubmitting"
        @click="open = false"
      />
      <UButton
        v-if="targetExists"
        color="warning"
        variant="soft"
        icon="i-lucide-replace"
        :label="t('paste.overwrite')"
        :disabled="isSubmitting"
        @click="submit(true)"
      />
      <UButton
        color="primary"
        icon="i-lucide-upload"
        :label="t('paste.save')"
        :loading="isSubmitting"
        :disabled="!canSubmit"
        @click="submit()"
      />
    </template>
  </UModal>
</template>

<style src="../../assets/css/clipboard-paste-dialog.css"></style>

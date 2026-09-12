<script setup>
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";

const props = defineProps({
  isDirectory: { type: Boolean, default: false },
  disabled: { type: Boolean, default: false },
});
const permissions = defineModel({ type: String, required: true });
const { t } = useI18n();
const roles = [
  { key: "permissionsOwner", shift: 6 },
  { key: "permissionsGroup", shift: 3 },
  { key: "permissionsOthers", shift: 0 },
];
const flags = computed(() => [
  { key: "permissionsRead", bit: 4, symbol: "r" },
  { key: "permissionsWrite", bit: 2, symbol: "w" },
  { key: props.isDirectory ? "permissionsTraverse" : "permissionsExecute", bit: 1, symbol: "x" },
]);
const numericMode = computed(() => /^[0-7]{3}$/.test(permissions.value)
  ? Number.parseInt(permissions.value, 8) : null);
const lastValidMode = ref(null);
// 数字框的编辑草稿可能暂时不完整，勾选仍应基于上一次有效权限，不能顺带清掉其他位。
watch(numericMode, (mode) => {
  if (mode !== null) lastValidMode.value = mode;
}, { immediate: true, flush: "sync" });
const modeError = computed(() => numericMode.value !== null ? undefined
  : t(lastValidMode.value === null ? "files.permissionsUnavailable" : "files.permissionsInvalid"));

// 勾选和数字输入共用同一份权限值，避免两套状态互相覆盖；缺失或未输入完整时不伪装成已取消权限。
const permissionRows = computed(() => roles.map((role) => ({
  key: role.key,
  label: t(`files.${role.key}`),
  options: flags.value.map((flag) => {
    const mask = flag.bit << role.shift;
    return {
      mask,
      label: t(`files.${flag.key}`),
      checked: lastValidMode.value === null ? "indeterminate" : (lastValidMode.value & mask) !== 0,
    };
  }),
})));
const symbolicMode = computed(() => {
  if (lastValidMode.value === null) return "";
  return roles.map((role) => flags.value
    .map((flag) => (lastValidMode.value & (flag.bit << role.shift)) !== 0 ? flag.symbol : "-")
    .join(""))
    .join("");
});

function updatePermission(mask, enabled) {
  if (props.disabled || lastValidMode.value === null) return;
  const current = lastValidMode.value;
  const next = enabled === true ? current | mask : current & ~mask;
  permissions.value = next.toString(8).padStart(3, "0");
}
</script>

<template>
  <div class="file-permissions-editor">
    <table class="file-permissions-table" :aria-label="t('files.changePermissionsTitle')">
      <colgroup>
        <col class="file-permissions-role-column" />
        <col v-for="flag in flags" :key="flag.bit" />
      </colgroup>
      <thead>
        <tr>
          <th scope="col">{{ t("files.permissionsRole") }}</th>
          <th v-for="flag in flags" :key="flag.bit" scope="col">{{ t(`files.${flag.key}`) }}</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="role in permissionRows" :key="role.key">
          <th scope="row">{{ role.label }}</th>
          <td v-for="option in role.options" :key="option.mask">
            <UCheckbox
              :model-value="option.checked"
              :disabled="disabled || lastValidMode === null"
              :aria-label="t('files.permissionsOption', { role: role.label, permission: option.label })"
              size="xl"
              :ui="{ root: 'justify-center', container: 'h-11', base: 'cursor-pointer disabled:cursor-not-allowed' }"
              @update:model-value="updatePermission(option.mask, $event)"
            />
          </td>
        </tr>
      </tbody>
    </table>

    <UFormField
      name="permissions"
      :label="t('files.permissionsOctal')"
      :error="modeError"
      :ui="{ labelWrapper: 'mb-2' }"
    >
      <div class="file-permissions-mode">
        <UInput
          v-model="permissions"
          class="file-permissions-input"
          :disabled="disabled"
          color="neutral"
          variant="outline"
          inputmode="numeric"
          maxlength="3"
          :ui="{ base: 'text-center font-mono tabular-nums rounded-lg' }"
        />
        <code class="file-permissions-symbolic">{{ symbolicMode }}</code>
      </div>
    </UFormField>
  </div>
</template>

<style scoped src="../assets/css/file-permissions-editor.css"></style>

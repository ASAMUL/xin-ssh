<script setup>
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { submitAuthenticationAnswers, useAuthenticationStore } from "../../stores/authenticationStore";

const { t } = useI18n();
const authentication = useAuthenticationStore();
const challenge = computed(() => authentication.activeChallenge);
const answers = ref([]);
const targetDescription = computed(() => {
  const target = challenge.value?.target;
  return target
    ? `${t(`authentication.purpose.${target.purpose}`)}: ${target.name}\n${target.username}@${target.host}:${target.port}`
    : "";
});

// 同一服务器也可能连续询问多轮；按 challengeId 清空，上一轮验证码绝不自动复用。
watch(() => challenge.value?.challengeId, () => {
  answers.value = challenge.value?.prompts.map(() => "") ?? [];
}, { immediate: true, flush: "sync" });

function submitAnswers() {
  if (!challenge.value || challenge.value.isSubmitting || !challenge.value.prompts.length) return;
  const challengeId = challenge.value.challengeId;
  const response = [...answers.value];
  answers.value = answers.value.map(() => "");
  submitAuthenticationAnswers(challengeId, response);
}

function cancelAuthentication() {
  if (!challenge.value || challenge.value.isSubmitting) return;
  answers.value = [];
  submitAuthenticationAnswers(challenge.value.challengeId, null);
}
</script>

<template>
  <UModal
    :open="Boolean(challenge)"
    :title="t('authentication.title')"
    :description="targetDescription"
    :dismissible="false"
    :close="false"
    :transition="false"
    :ui="{ wrapper: 'min-w-0', description: 'whitespace-pre-line break-all', footer: 'justify-end flex-wrap' }"
  >
    <template #body>
      <form v-if="challenge" id="authentication-response" :key="challenge.challengeId"
        class="authentication-form" autocomplete="off" @submit.prevent="submitAnswers">
        <strong v-if="challenge.name" class="authentication-server-text">{{ challenge.name }}</strong>
        <p v-if="challenge.instructions" class="authentication-server-text">{{ challenge.instructions }}</p>
        <UFormField v-for="(prompt, index) in challenge.prompts" :key="index"
          :label="prompt.prompt || t('authentication.response', { index: index + 1 })"
          :ui="{ label: 'whitespace-pre-wrap break-all' }">
          <UInput v-model="answers[index]" :type="prompt.echo ? 'text' : 'password'"
            :autofocus="index === 0" :disabled="challenge.isSubmitting" size="lg" class="w-full"
            autocomplete="off" :spellcheck="false" autocapitalize="off" :maxlength="16384" />
        </UFormField>
        <p v-if="!challenge.prompts.length" class="authentication-waiting">
          <UIcon name="i-lucide-loader-circle" class="animate-spin" />
          {{ t('authentication.waiting') }}
        </p>
        <UAlert v-if="challenge.error" color="error" :title="challenge.error" />
      </form>
    </template>
    <template #footer>
      <UButton color="neutral" variant="outline" :label="t('common.cancel')"
        :disabled="challenge?.isSubmitting" @click="cancelAuthentication" />
      <UButton v-if="challenge?.prompts.length" type="submit" form="authentication-response"
        icon="i-lucide-arrow-right" :label="t('authentication.continue')" :loading="challenge.isSubmitting" />
    </template>
  </UModal>
</template>

<style scoped src="../../assets/css/authentication-dialog.css"></style>

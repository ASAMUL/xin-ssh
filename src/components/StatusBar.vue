<script setup>
defineProps({
  connection: {
    type: Object,
    default: null,
  },
  selectedFile: {
    type: Object,
    default: null,
  },
  summary: {
    type: Object,
    required: true,
  },
});
</script>

<template>
  <footer class="status-bar" aria-label="Connection status">
    <div class="status-cluster">
      <span class="status-pill">
        <span class="status-dot"></span>
        Connected
      </span>
      <span class="status-pill">
        <UIcon name="i-lucide-lock-keyhole" />
        SSH
      </span>
      <span class="status-pill endpoint">{{ connection?.host }}</span>
    </div>

    <div class="status-cluster right">
      <span class="status-pill selected-file" :title="selectedFile?.permission">
        <UIcon name="i-lucide-folder" />
        {{ summary.protocol }}
      </span>
      <span class="status-pill upload">
        <UIcon name="i-lucide-arrow-up" />
        {{ summary.uploadSpeed }}
      </span>
      <span class="status-pill download">
        <UIcon name="i-lucide-arrow-down" />
        {{ summary.downloadSpeed }}
      </span>
      <span class="status-pill">
        Ping: {{ summary.ping }}
        <span class="status-dot"></span>
      </span>
      <span class="status-pill">
        <UIcon name="i-lucide-activity" />
      </span>
    </div>
  </footer>
</template>

<style scoped>
.status-bar {
  height: 49px;
  min-height: 49px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 18px;
  padding: 0 24px;
  color: #0f172a;
  border-top: 1px solid rgba(148, 163, 184, 0.22);
  background: rgba(255, 255, 255, 0.88);
}

.status-cluster {
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 18px;
}

.status-cluster.right {
  justify-content: flex-end;
}

.status-pill {
  min-width: 0;
  display: inline-flex;
  align-items: center;
  gap: 8px;
  color: #0f172a;
  font-size: 13px;
  white-space: nowrap;
}

.endpoint {
  overflow: hidden;
  max-width: 220px;
  text-overflow: ellipsis;
}

.status-dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background: #22c55e;
}

.upload {
  color: #0757e6;
}

.download {
  color: #16a34a;
}

@media (max-width: 1120px) {
  .selected-file,
  .status-pill:nth-child(4) {
    display: none;
  }
}
</style>

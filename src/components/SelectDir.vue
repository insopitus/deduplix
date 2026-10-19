<script setup lang="ts">
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen, FolderSearch, ListFilter } from "@lucide/vue";
import { Ref, ref } from "vue";
import LabeledInput from "./LabeledInput.vue";
import SizeUnitRadio from "./SizeUnitRadio.vue";
import { type SizeUnit, type PathConfig } from "../types";

const emit = defineEmits<{ "path-selected": [config: PathConfig] }>();

const KILO_BYTE = 1024;
const MEGA_BYTE = KILO_BYTE * 1024;
const GIGA_BYTE = MEGA_BYTE * 1024;

const include = ref("");
const exclude = ref("");
const quickMode = ref(false);
const minSize = ref(1);
const minSizeUnit: Ref<SizeUnit> = ref("KB");
const maxSize = ref(10);
const maxSizeUnit: Ref<SizeUnit> = ref("GB");

const MIN_SIZE_VALUE = 0;
const MAX_SIZE_VALUE = 999999;

async function chooseDirectory() {
  validateSize();
  try {
    const path = await open({
      multiple: false,
      directory: true,
    });
    if (path) {
      emit("path-selected", {
        path,
        config: {
          include: include.value.trim(),
          exclude: exclude.value.trim(),
          size_extend: [
            convertSize(minSize.value, minSizeUnit.value),
            convertSize(maxSize.value, maxSizeUnit.value),
          ],
          quick_mode: quickMode.value,
        },
      });
    }
  } catch (e) {
    console.error(e);
  }
}

function clampSize(value: unknown, fallback: number) {
  const n = Number(value);
  if (!Number.isFinite(n)) return fallback;
  return Math.min(MAX_SIZE_VALUE, Math.max(MIN_SIZE_VALUE, Math.round(n)));
}

function validateSize() {
  minSize.value = clampSize(minSize.value, MIN_SIZE_VALUE);
  maxSize.value = clampSize(maxSize.value, minSize.value);
  if (maxSize.value < minSize.value) {
    maxSize.value = minSize.value;
  }
}

function convertSize(num: number, unit: SizeUnit) {
  const value = Number(num);
  if (!Number.isFinite(value)) return 0;
  switch (unit) {
    case "B":
      return value;
    case "KB":
      return value * KILO_BYTE;
    case "MB":
      return value * MEGA_BYTE;
    case "GB":
      return value * GIGA_BYTE;
    default:
      return 0;
  }
}
</script>

<template>
  <div class="home">
    <div class="main">
      <div class="hero">
        <div class="hero-mark" aria-hidden="true">
          <FolderSearch :size="22" :stroke-width="1.8" />
        </div>
        <h1>Find duplicate files</h1>
        <p class="muted">
          Scan a folder for redundant copies. Everything stays on this device.
        </p>
      </div>

      <label class="mode-toggle">
        <span class="mode-text">
          <span class="mode-title">Quick mode</span>
          <span class="mode-hint">
            Skip full hashing. Groups are matched by a 1 KB sample and may be
            wrong.
          </span>
        </span>
        <span class="switch">
          <input v-model="quickMode" type="checkbox" />
          <span class="switch-track" aria-hidden="true">
            <span class="switch-thumb"></span>
          </span>
        </span>
      </label>

      <section class="filters">
        <div class="filters-head">
          <h2>
            <ListFilter :size="14" :stroke-width="2" aria-hidden="true" />
            Filters
          </h2>
          <span>optional</span>
        </div>

        <div class="field-grid">
          <LabeledInput
            v-model="include"
            label="Include"
            placeholder="**/*.txt, **/*.mp4"
          />
          <LabeledInput
            v-model="exclude"
            label="Exclude"
            placeholder="**/node_modules, **/*.tmp"
          />
        </div>
        <p class="hint">
          Glob patterns separated by commas. Leave empty to include everything.
        </p>

        <div class="size-line">
          <LabeledInput
            v-model="minSize"
            label="Min size"
            type="number"
            :min="MIN_SIZE_VALUE"
            :max="MAX_SIZE_VALUE"
            @change="validateSize"
            placeholder="1"
          >
            <SizeUnitRadio
              v-model="minSizeUnit"
              name="min-size"
              label="Minimum size unit"
              :options="['B', 'KB', 'MB', 'GB']"
              embedded
            />
          </LabeledInput>
          <LabeledInput
            v-model="maxSize"
            label="Max size"
            type="number"
            :min="MIN_SIZE_VALUE"
            :max="MAX_SIZE_VALUE"
            @change="validateSize"
            placeholder="10"
          >
            <SizeUnitRadio
              v-model="maxSizeUnit"
              name="max-size"
              label="Maximum size unit"
              :options="['B', 'KB', 'MB', 'GB']"
              embedded
            />
          </LabeledInput>
        </div>
      </section>

      <div class="start-action">
        <button class="primary" type="button" @click="chooseDirectory">
          <FolderOpen :size="18" :stroke-width="2" aria-hidden="true" />
          Choose a directory to start…
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.home {
  width: 100%;
  max-width: 720px;
  margin: 0 auto;
}

.main {
  width: 100%;
  background: var(--card);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  padding: 26px;
  box-shadow: var(--shadow);
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.hero {
  margin-bottom: 4px;
}

.hero-mark {
  display: grid;
  place-items: center;
  width: 44px;
  height: 44px;
  margin-bottom: 12px;
  border: 1px solid var(--border);
  border-radius: 13px;
  background: var(--accent-soft);
  color: var(--accent-deep);
}

.hero h1 {
  margin: 0 0 4px;
  font-size: 24px;
  line-height: 1.2;
  letter-spacing: -0.02em;
}

.muted {
  color: var(--muted);
  margin: 0;
  font-size: 13.5px;
}

button.primary {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 9px;
  width: 100%;
  appearance: none;
  border: none;
  background: linear-gradient(135deg, var(--accent-fill), var(--accent-fill-2));
  color: var(--on-accent);
  padding: 13px 16px;
  font-size: 15px;
  font-weight: 600;
  border-radius: 11px;
  cursor: pointer;
  box-shadow: 0 10px 24px rgba(53, 103, 224, 0.24);
  transition:
    transform 0.12s ease,
    box-shadow 0.12s ease,
    filter 0.12s ease;
}

button.primary:hover {
  transform: translateY(-1px);
  box-shadow: 0 14px 30px rgba(53, 103, 224, 0.3);
  filter: brightness(1.05);
}

button.primary:active {
  transform: translateY(0);
}

.start-action {
  margin-top: 6px;
  padding-top: 18px;
  border-top: 1px solid var(--border);
}

.mode-toggle {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-top: 2px;
  padding: 10px 12px;
  border: 1px solid var(--border);
  border-radius: 10px;
  background: var(--card-subtle);
  cursor: pointer;
}

.mode-text {
  display: flex;
  flex-direction: column;
  gap: 1px;
  min-width: 0;
}

.mode-title {
  font-size: 13px;
  font-weight: 600;
}

.mode-hint {
  font-size: 12px;
  color: var(--muted);
}

.switch {
  position: relative;
  display: inline-flex;
  flex: none;
}

.switch input {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  margin: 0;
  opacity: 0;
  cursor: pointer;
}

.switch-track {
  display: block;
  width: 36px;
  height: 20px;
  padding: 2px;
  border-radius: 999px;
  background: var(--border-strong);
  transition: background 0.15s ease;
}

.switch-thumb {
  display: block;
  width: 16px;
  height: 16px;
  border-radius: 50%;
  background: var(--card);
  box-shadow: var(--shadow-sm);
  transition: transform 0.15s ease;
}

.switch input:checked + .switch-track {
  background: var(--accent);
}

.switch input:checked + .switch-track .switch-thumb {
  transform: translateX(16px);
}

.switch input:focus-visible + .switch-track {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}

.filters {
  display: flex;
  flex-direction: column;
  gap: 10px;
  margin-top: 6px;
  padding-top: 16px;
  border-top: 1px solid var(--border);
}

.filters-head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 12px;
}

.filters-head h2 {
  display: flex;
  align-items: center;
  gap: 6px;
  margin: 0;
  font-size: 12px;
  font-weight: 700;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: var(--muted);
}

.filters-head span {
  font-size: 12px;
  color: var(--muted);
}

.field-grid,
.size-line {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
}

.field-grid :deep(.labeled-input),
.size-line :deep(.labeled-input) {
  margin-top: 0;
}

.hint {
  margin: -2px 0 0;
  font-size: 12px;
  color: var(--muted);
}

@media (max-width: 560px) {
  .main {
    padding: 20px;
  }
  .hero h1 {
    font-size: 21px;
  }
  .field-grid {
    grid-template-columns: minmax(0, 1fr);
  }
}
</style>

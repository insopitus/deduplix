<script setup lang="ts">
import { computed } from "vue";
import type { AnalysisStage } from "../types";

const props = defineProps<{
    stages: AnalysisStage[];
    stageIndex: number;
}>();

const progress = computed(() =>
    Math.min(
        100,
        Math.max(6, (props.stageIndex / props.stages.length) * 100),
    ),
);
const currentStage = computed<AnalysisStage | undefined>(
    () => props.stages[props.stageIndex],
);
</script>

<template>
    <div class="progress-wrap" role="status" aria-live="polite">
        <div class="progress-head">
            <div class="progress-title">
                <span
                    v-if="currentStage"
                    class="spinner"
                    aria-hidden="true"
                ></span>
                <span>{{ currentStage ? currentStage.label : "Finishing up" }}</span>
                <span v-if="currentStage" class="hint">
                    {{ currentStage.hint }}
                </span>
            </div>
            <span class="percent">{{ Math.round(progress) }}%</span>
        </div>

        <div
            class="bar"
            role="progressbar"
            aria-label="Analysis progress"
            aria-valuemin="0"
            aria-valuemax="100"
            :aria-valuenow="Math.round(progress)"
        >
            <div class="bar-fill" :style="{ width: progress + '%' }">
                <span
                    v-if="currentStage"
                    class="shimmer"
                    aria-hidden="true"
                ></span>
            </div>
        </div>

        <ol class="steps">
            <li
                v-for="(stage, index) in stages"
                :key="stage.event"
                :class="{
                    done: index < stageIndex,
                    active: index === stageIndex,
                }"
            >
                <span class="step-dot" aria-hidden="true">
                    <span v-if="index < stageIndex">✓</span>
                </span>
                <span class="step-text">
                    <span class="step-label">{{ stage.label }}</span>
                    <span class="step-hint">{{ stage.hint }}</span>
                </span>
            </li>
        </ol>
    </div>
</template>

<style scoped>
.progress-wrap {
    display: flex;
    flex-direction: column;
    gap: 12px;
    width: 100%;
}

.progress-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
}

.progress-title {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    font-size: 14px;
    font-weight: 600;
}

.progress-title .hint {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12px;
    font-weight: 400;
    color: var(--muted);
}

.percent {
    flex: none;
    font-size: 12px;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
}

.spinner {
    flex: none;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    border: 2px solid var(--accent-soft);
    border-top-color: var(--accent);
    animation: spin 0.8s linear infinite;
}

.bar {
    position: relative;
    width: 100%;
    height: 7px;
    overflow: hidden;
    border-radius: 999px;
    background: var(--border);
}

.bar-fill {
    position: relative;
    height: 100%;
    overflow: hidden;
    border-radius: 999px;
    background: linear-gradient(90deg, var(--accent), var(--accent-2));
    transition: width 0.35s ease;
}

.shimmer {
    position: absolute;
    inset: 0;
    background: linear-gradient(
        90deg,
        transparent,
        rgba(255, 255, 255, 0.45),
        transparent
    );
    transform: translateX(-100%);
    animation: shimmer 1.4s ease-in-out infinite;
}

.steps {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 8px;
    margin: 2px 0 0;
    padding: 0;
    list-style: none;
}

.steps li {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    padding: 10px;
    border: 1px solid var(--border);
    border-radius: 10px;
    opacity: 0.5;
    transition:
        opacity 0.2s ease,
        border-color 0.2s ease,
        background 0.2s ease;
}

.steps li.active {
    opacity: 1;
    border-color: var(--accent);
    background: var(--accent-soft);
}

.steps li.done {
    opacity: 1;
}

.step-dot {
    display: grid;
    place-items: center;
    flex: none;
    width: 17px;
    height: 17px;
    margin-top: 1px;
    border-radius: 50%;
    border: 2px solid var(--border-strong);
    font-size: 10px;
    font-weight: 700;
    color: var(--on-accent);
    transition:
        background 0.2s ease,
        border-color 0.2s ease;
}

.active .step-dot {
    border-color: var(--accent);
    border-right-color: transparent;
    animation: spin 0.8s linear infinite;
}

.done .step-dot {
    background: var(--success);
    border-color: var(--success);
}

.step-text {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
}

.step-label {
    font-size: 12.5px;
    font-weight: 600;
}

.step-hint {
    font-size: 11px;
    color: var(--muted);
}

@keyframes spin {
    to {
        transform: rotate(360deg);
    }
}

@keyframes shimmer {
    0% {
        transform: translateX(-100%);
    }
    60%,
    100% {
        transform: translateX(100%);
    }
}

@media (max-width: 640px) {
    .steps {
        grid-template-columns: minmax(0, 1fr);
    }
    .progress-title .hint {
        display: none;
    }
}
</style>

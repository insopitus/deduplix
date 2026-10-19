<script setup lang="ts">
import { Check } from "@lucide/vue";
import { computed } from "vue";
import type { AnalysisStage, StageProgress } from "../types";

const props = defineProps<{
    stages: AnalysisStage[];
    stageIndex: number;
    progress: (StageProgress | null)[];
}>();

const activeStage = computed<AnalysisStage | undefined>(
    () => props.stages[props.stageIndex],
);

/**
 * `null` means the stage has no measurable total yet and should animate.
 */
function percentOf(index: number): number | null {
    if (index < props.stageIndex) return 100;
    if (index > props.stageIndex) return 0;
    const progress = props.progress[index];
    if (!progress) return null;
    if (progress.total <= 0) return 100;
    return Math.min(100, (progress.done / progress.total) * 100);
}

function isIndeterminate(index: number) {
    return index === props.stageIndex && percentOf(index) === null;
}

const activePercent = computed(() =>
    props.stageIndex < props.stages.length
        ? percentOf(props.stageIndex)
        : null,
);
</script>

<template>
    <div class="progress-wrap" role="status" aria-live="polite">
        <div class="progress-head">
            <div class="progress-title">
                <span
                    v-if="activeStage"
                    class="spinner"
                    aria-hidden="true"
                ></span>
                <span>{{ activeStage ? activeStage.label : "Finishing up" }}</span>
                <span v-if="activeStage" class="hint">
                    {{ activeStage.hint }}
                </span>
            </div>
            <span v-if="activePercent !== null" class="percent">
                {{ Math.round(activePercent) }}%
            </span>
        </div>

        <ol class="steps" :style="{ '--stage-count': stages.length }">
            <li
                v-for="(stage, index) in stages"
                :key="stage.event"
                :class="{
                    done: index < stageIndex,
                    active: index === stageIndex,
                }"
            >
                <span class="step-dot" aria-hidden="true">
                    <Check
                        v-if="index < stageIndex"
                        :size="10"
                        :stroke-width="3"
                    />
                </span>

                <div class="step-body">
                    <div class="step-text">
                        <span class="step-label">
                            <component
                                :is="stage.icon"
                                :size="13"
                                aria-hidden="true"
                            />
                            {{ stage.label }}
                        </span>
                        <span class="step-hint">{{ stage.hint }}</span>
                    </div>
                    <div
                        class="bar"
                        role="progressbar"
                        :aria-label="`${stage.label} progress`"
                        aria-valuemin="0"
                        aria-valuemax="100"
                        :aria-valuenow="
                            isIndeterminate(index)
                                ? undefined
                                : Math.round(percentOf(index) ?? 0)
                        "
                    >
                        <div
                            v-if="isIndeterminate(index)"
                            class="bar-fill indeterminate"
                        ></div>
                        <div
                            v-else
                            class="bar-fill"
                            :style="{ width: (percentOf(index) ?? 0) + '%' }"
                        ></div>
                    </div>
                </div>

                <span
                    v-if="index === stageIndex && percentOf(index) !== null"
                    class="step-percent"
                >
                    {{ Math.round(percentOf(index) ?? 0) }}%
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

.steps {
    display: grid;
    grid-template-columns: repeat(var(--stage-count, 3), minmax(0, 1fr));
    gap: 8px;
    margin: 2px 0 0;
    padding: 0;
    list-style: none;
}

.steps li {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: start;
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

.step-body {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
}

.step-text {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
}

.step-label {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 12.5px;
    font-weight: 600;
}

.step-label :deep(svg) {
    flex: none;
    opacity: 0.8;
}

.step-hint {
    font-size: 11px;
    color: var(--muted);
}

.step-percent {
    font-size: 11px;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
}

.bar {
    position: relative;
    width: 100%;
    height: 5px;
    overflow: hidden;
    border-radius: 999px;
    background: var(--border);
}

.bar-fill {
    height: 100%;
    border-radius: 999px;
    background: linear-gradient(90deg, var(--accent), var(--accent-2));
    transition: width 0.25s linear;
}

.done .bar-fill {
    background: var(--success);
}

.bar-fill.indeterminate {
    width: 35%;
    animation: indeterminate 1.1s ease-in-out infinite;
}

@keyframes spin {
    to {
        transform: rotate(360deg);
    }
}

@keyframes indeterminate {
    0% {
        transform: translateX(-100%);
    }
    100% {
        transform: translateX(285%);
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

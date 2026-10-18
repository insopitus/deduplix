<script lang="ts">
// Module-level so every mounted results screen gets a unique analysis id.
let nextAnalysisId = 0;
</script>

<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { computed, onUnmounted, ref } from "vue";
import { type AnalysisStage } from "../types";
import AnalysisProgress from "./AnalysisProgress.vue";
import DupGroup from "./DupGroup.vue";

type Pair = {
    hash: string;
    size: string;
    size_bytes: number;
    files: string[];
};

const props = defineProps<{
    workingPath: string;
    config: {
        include: string;
        exclude: string;
        size_extend: [number, number];
    };
}>();
const emit = defineEmits<{ back: [] }>();

const stages: AnalysisStage[] = [
    {
        event: "ingestion-done",
        label: "Scanning folder",
        hint: "Building the size index",
    },
    {
        event: "quick-hash-done",
        label: "Quick filtering",
        hint: "Comparing 1 KB samples",
    },
    {
        event: "full-hash-done",
        label: "Hashing files",
        hint: "Verifying full contents",
    },
];

const analysisDone = ref(false);
const analysisError = ref("");
const actionError = ref("");
const skipped = ref<string[]>([]);
const pairs = ref<Pair[]>([]);
const stageIndex = ref(0);
const cancelling = ref(false);
const analysisId = ++nextAnalysisId;

let unsubs: UnlistenFn[] = [];
let disposed = false;

function cleanupListeners() {
    for (const unsub of unsubs) {
        try {
            unsub();
        } catch (e) {
            // ignore
        }
    }
    unsubs = [];
}

function unsubscribeAll(listeners: Array<UnlistenFn | undefined>) {
    for (const unsub of listeners) {
        try {
            unsub?.();
        } catch (e) {
            // ignore
        }
    }
}

async function runAnalysis() {
    // Subscribe before invoking the command. A small directory can finish
    // before the webview has registered its listeners, which used to leave
    // the progress bar stuck at 0%.
    const listeners = await Promise.all(
        stages.map((stage, index) =>
            listen(stage.event, () => {
                stageIndex.value = Math.max(stageIndex.value, index + 1);
            }).catch(() => undefined),
        ),
    );
    if (disposed) {
        unsubscribeAll(listeners);
        return;
    }
    // The user may press Cancel while we are still registering listeners.
    // In that case the backend analysis has not started yet, so simply leave.
    if (cancelling.value) {
        unsubscribeAll(listeners);
        emit("back");
        return;
    }
    unsubs = listeners.filter(
        (unsub): unsub is UnlistenFn => Boolean(unsub),
    );

    try {
        const result = await invoke<{
            pairs: Pair[];
            skipped?: string[];
            cancelled?: boolean;
        }>("start_analysis", {
            id: analysisId,
            path: props.workingPath,
            config: props.config,
        });
        if (result.cancelled || cancelling.value) {
            emit("back");
            return;
        }
        pairs.value = result.pairs ?? [];
        skipped.value = result.skipped ?? [];
    } catch (error) {
        if (cancelling.value) {
            // The user asked to leave; do not keep them on the results screen
            // because the backend failed while cancelling.
            emit("back");
        } else {
            analysisError.value = String(error);
        }
    } finally {
        analysisDone.value = true;
        cleanupListeners();
    }
}

async function requestCancel() {
    try {
        await invoke("cancel_analysis", { id: analysisId });
    } catch (error) {
        if (!analysisDone.value) {
            cancelling.value = false;
            actionError.value = `Could not cancel the analysis: ${error}`;
        }
    }
}

function handleBack() {
    if (analysisDone.value) {
        emit("back");
        return;
    }
    if (cancelling.value) return;
    cancelling.value = true;
    void requestCancel();
}

onUnmounted(() => {
    disposed = true;
    cleanupListeners();
});

runAnalysis();

const groups = computed(() =>
    pairs.value.filter((pair) => pair.files.length > 1),
);
const redundantFiles = computed(() =>
    groups.value.reduce((total, group) => total + group.files.length - 1, 0),
);
const reclaimable = computed(() =>
    formatBytes(
        groups.value.reduce(
            (total, group) =>
                total + (group.size_bytes ?? 0) * (group.files.length - 1),
            0,
        ),
    ),
);

const UNITS = ["B", "KB", "MB", "GB", "TB"];

function formatBytes(bytes: number) {
    if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
    const exponent = Math.min(
        Math.floor(Math.log(bytes) / Math.log(1024)),
        UNITS.length - 1,
    );
    const value = bytes / 1024 ** exponent;
    const digits = value >= 100 ? 0 : value >= 10 ? 1 : 2;
    return `${value.toFixed(digits)} ${UNITS[exponent]}`;
}

</script>

<template>
    <div class="results">
        <header class="top-bar">
            <button
                class="back"
                :class="{ cancel: !analysisDone }"
                type="button"
                :disabled="cancelling"
                :title="
                    analysisDone
                        ? 'Back to folder selection'
                        : 'Cancel analysis'
                "
                @click="handleBack"
            >
                <span aria-hidden="true">{{ analysisDone ? "←" : "×" }}</span>
                {{ cancelling ? "Cancelling…" : analysisDone ? "Back" : "Cancel" }}
            </button>
            <div class="path" :title="workingPath">
                <svg
                    width="15"
                    height="15"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    aria-hidden="true"
                >
                    <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
                </svg>
                <span class="path-label">{{
                    cancelling
                        ? "Cancelling"
                        : analysisDone
                          ? "Scanned"
                          : "Scanning"
                }}</span>
                <span class="path-value">{{ workingPath }}</span>
            </div>
        </header>

        <div
            v-if="actionError"
            class="banner error"
            role="alert"
        >
            <span>{{ actionError }}</span>
            <button
                type="button"
                class="dismiss"
                aria-label="Dismiss error"
                @click="actionError = ''"
            >
                ×
            </button>
        </div>

        <div
            v-if="skipped.length"
            class="banner warning"
            role="status"
            :title="skipped.join('\n')"
        >
            <span>
                {{ skipped.length }}
                {{ skipped.length === 1 ? "file" : "files" }} couldn't be
                read and {{ skipped.length === 1 ? "was" : "were" }} skipped.
            </span>
        </div>

        <p v-if="analysisError" class="banner error" role="alert">
            Analysis failed: {{ analysisError }}
        </p>

        <template v-else>
            <div v-if="!analysisDone" class="progress-card">
                <AnalysisProgress
                    :stages="stages"
                    :stage-index="stageIndex"
                />
            </div>

            <template v-else>
                <div v-if="groups.length" class="summary">
                    <div class="stat">
                        <strong>{{ groups.length }}</strong>
                        <span>duplicate groups</span>
                    </div>
                    <div class="stat">
                        <strong>{{ redundantFiles }}</strong>
                        <span>redundant files</span>
                    </div>
                    <div class="stat accent">
                        <strong>{{ reclaimable }}</strong>
                        <span>can be reclaimed</span>
                    </div>
                </div>

                <div v-if="groups.length === 0" class="no-result">
                    <div class="no-result-icon" aria-hidden="true">✓</div>
                    <h2>No duplicates found</h2>
                    <p>
                        This folder looks clean. Try widening the size range or
                        removing a filter.
                    </p>
                </div>

                <section v-else class="group-list">
                    <DupGroup
                        v-for="pair in groups"
                        :key="pair.hash + pair.size_bytes"
                        :root-path="workingPath"
                        :item="pair"
                        @error="actionError = $event"
                    />
                </section>
            </template>
        </template>
    </div>
</template>

<style lang="css" scoped>
.results {
    width: 100%;
    max-width: 960px;
    margin: 0 auto;
    display: flex;
    flex-direction: column;
    gap: 14px;
}

.top-bar {
    position: sticky;
    top: 0;
    z-index: 20;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 0;
    margin-top: -10px;
    background: var(--bg-translucent);
    backdrop-filter: blur(10px);
}

button.back {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    flex: none;
    height: 42px;
    padding: 0 12px;
    border-radius: 9px;
    border: 1px solid var(--border);
    background: var(--card);
    color: var(--text);
    cursor: pointer;
    font-size: 13px;
    font-weight: 600;
    box-shadow: var(--shadow-sm);
    transition:
        border-color 0.12s ease,
        background 0.12s ease;
}

button.back:hover:not(:disabled) {
    border-color: var(--border-strong);
    background: var(--card-subtle);
}

button.back:disabled {
    opacity: 0.55;
    cursor: not-allowed;
}

button.back.cancel {
    color: var(--danger);
}

button.back.cancel:hover:not(:disabled) {
    border-color: var(--danger-border);
    background: var(--danger-soft);
}

.path {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1 1 auto;
    min-width: 0;
    height: 42px;
    padding: 0 12px;
    border: 1px solid var(--border);
    border-radius: 9px;
    background: var(--card);
    box-shadow: var(--shadow-sm);
    color: var(--muted);
}

.path svg {
    flex: none;
    color: var(--muted);
}

.path-label {
    flex: none;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--muted);
    padding-right: 8px;
    border-right: 1px solid var(--border);
}

.path-value {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 13px;
    color: var(--text);
}

.banner {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
    margin: 0;
    padding: 11px 14px;
    border-radius: 10px;
    font-size: 13px;
}

.banner.error {
    background: var(--danger-soft);
    border: 1px solid var(--danger-border);
    color: var(--danger);
}

.banner.warning {
    background: var(--warning-soft);
    border: 1px solid var(--warning-border);
    color: var(--warning);
}

.banner .dismiss {
    flex: none;
    border: none;
    background: transparent;
    color: inherit;
    font-size: 18px;
    line-height: 1;
    padding: 0 2px;
    cursor: pointer;
}

.progress-card {
    padding: 20px;
    background: var(--card);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    box-shadow: var(--shadow-sm);
}

.summary {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 10px;
}

.stat {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 13px 16px;
    background: var(--card);
    border: 1px solid var(--border);
    border-radius: 11px;
    box-shadow: var(--shadow-sm);
}

.stat strong {
    font-size: 20px;
    line-height: 1.25;
    letter-spacing: -0.02em;
    font-variant-numeric: tabular-nums;
}

.stat span {
    font-size: 12px;
    color: var(--muted);
}

.stat.accent strong {
    color: var(--accent-deep);
}

.group-list {
    display: flex;
    flex-direction: column;
    gap: 12px;
}

.no-result {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 6px;
    text-align: center;
    padding: 52px 24px;
    background: var(--card);
    border: 1px dashed var(--border-strong);
    border-radius: var(--radius);
}

.no-result-icon {
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    margin-bottom: 6px;
    border-radius: 50%;
    background: var(--success-soft);
    color: var(--success);
    font-size: 20px;
    font-weight: 700;
}

.no-result h2 {
    margin: 0;
    font-size: 17px;
}

.no-result p {
    margin: 0;
    font-size: 13px;
    color: var(--muted);
    max-width: 34ch;
}

@media (max-width: 560px) {
    .summary {
        grid-template-columns: minmax(0, 1fr);
    }
    .path-label {
        display: none;
    }
    .stat {
        flex-direction: row;
        align-items: baseline;
        gap: 8px;
    }
}
</style>

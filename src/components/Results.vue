<script lang="ts">
// Module-level so every mounted results screen gets a unique analysis id.
let nextAnalysisId = 0;
</script>

<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import {
    ArrowLeft,
    Check,
    CircleAlert,
    Copy,
    Files,
    FolderOpen,
    FolderSearch,
    HardDrive,
    Hash,
    ScanSearch,
    TriangleAlert,
    X,
} from "@lucide/vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { computed, onUnmounted, ref } from "vue";
import { type AnalysisStage, type StageProgress } from "../types";
import AnalysisProgress from "./AnalysisProgress.vue";
import DupGroup from "./DupGroup.vue";

type Pair = {
    hash: string;
    size: string;
    size_bytes: number;
    files: string[];
    not_duplicates?: boolean;
};

const props = defineProps<{
    workingPath: string;
    config: {
        include: string;
        exclude: string;
        size_extend: [number, number];
        quick_mode: boolean;
    };
}>();
const emit = defineEmits<{ back: [] }>();

const stages: AnalysisStage[] = [
    {
        event: "ingestion-done",
        label: "Scanning folder",
        hint: "Building the size index",
        icon: FolderSearch,
    },
    {
        event: "quick-hash-done",
        label: "Quick filtering",
        hint: "Comparing 4 KB samples",
        icon: ScanSearch,
    },
    {
        event: "full-hash-done",
        label: "Hashing files",
        hint: "Verifying full contents",
        icon: Hash,
    },
];

const quickMode = computed(() => props.config.quick_mode);
const activeStages = computed(() =>
    quickMode.value ? stages.slice(0, 2) : stages,
);

const quickProgress = ref<StageProgress | null>(null);
const fullProgress = ref<StageProgress | null>(null);
const stageProgress = computed<(StageProgress | null)[]>(() =>
    quickMode.value
        ? [null, quickProgress.value]
        : [null, quickProgress.value, fullProgress.value],
);

function mergeProgress(
    current: StageProgress | null,
    next: StageProgress,
): StageProgress {
    return {
        total: next.total,
        done: Math.max(current?.done ?? 0, next.done),
    };
}

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
    const listeners = await Promise.all([
        ...activeStages.value.map((stage, index) =>
            listen(stage.event, () => {
                stageIndex.value = Math.max(stageIndex.value, index + 1);
            }).catch(() => undefined),
        ),
        listen<StageProgress>("quick-hash-progress", (event) => {
            quickProgress.value = mergeProgress(
                quickProgress.value,
                event.payload,
            );
        }).catch(() => undefined),
        listen<StageProgress>("full-hash-progress", (event) => {
            fullProgress.value = mergeProgress(
                fullProgress.value,
                event.payload,
            );
        }).catch(() => undefined),
    ]);
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

// Verified groups that turned out not to be duplicates keep their card,
// even though their file list is now empty.
const groups = computed(() =>
    pairs.value.filter(
        (pair) => pair.not_duplicates || pair.files.length > 1,
    ),
);
const duplicateGroups = computed(() =>
    groups.value.filter((group) => group.files.length > 1),
);
const redundantFiles = computed(() =>
    groups.value.reduce(
        (total, group) => total + Math.max(0, group.files.length - 1),
        0,
    ),
);
const reclaimable = computed(() =>
    formatBytes(
        groups.value.reduce(
            (total, group) =>
                total +
                (group.size_bytes ?? 0) * Math.max(0, group.files.length - 1),
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
                <ArrowLeft
                    v-if="analysisDone"
                    :size="15"
                    aria-hidden="true"
                />
                <X v-else :size="15" aria-hidden="true" />
                {{ cancelling ? "Cancelling…" : analysisDone ? "Back" : "Cancel" }}
            </button>
            <div class="path" :title="workingPath">
                <FolderOpen :size="15" aria-hidden="true" />
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

        <div v-if="actionError" class="banner error" role="alert">
            <span class="banner-message">
                <CircleAlert :size="15" aria-hidden="true" />
                <span>{{ actionError }}</span>
            </span>
            <button
                type="button"
                class="dismiss"
                aria-label="Dismiss error"
                @click="actionError = ''"
            >
                <X :size="15" aria-hidden="true" />
            </button>
        </div>

        <div
            v-if="skipped.length"
            class="banner warning"
            role="status"
            :title="skipped.join('\n')"
        >
            <span class="banner-message">
                <TriangleAlert :size="15" aria-hidden="true" />
                <span>
                    {{ skipped.length }}
                    {{ skipped.length === 1 ? "file" : "files" }} couldn't
                    be read and
                    {{ skipped.length === 1 ? "was" : "were" }} skipped.
                </span>
            </span>
        </div>

        <div v-if="analysisError" class="banner error" role="alert">
            <span class="banner-message">
                <CircleAlert :size="15" aria-hidden="true" />
                <span>Analysis failed: {{ analysisError }}</span>
            </span>
        </div>

        <template v-else>
            <div v-if="quickMode" class="banner warning" role="status">
                <span class="banner-message">
                    <TriangleAlert :size="15" aria-hidden="true" />
                    <span>
                        <strong>Quick mode:</strong> files larger than
                        4&nbsp;KB are matched by a sample hash, so those groups
                        are not guaranteed to be accurate. Files up to
                        4&nbsp;KB are fully hashed. Run a full hash on a sampled
                        group before deleting files.
                    </span>
                </span>
            </div>
            <div v-if="!analysisDone" class="progress-card">
                <AnalysisProgress
                    :stages="activeStages"
                    :stage-index="stageIndex"
                    :progress="stageProgress"
                />
            </div>

            <template v-else>
                <div v-if="groups.length" class="summary">
                    <div class="stat">
                        <strong>{{ duplicateGroups.length }}</strong>
                        <span class="stat-label">
                            <Copy :size="13" aria-hidden="true" />
                            duplicate groups
                        </span>
                    </div>
                    <div class="stat">
                        <strong>{{ redundantFiles }}</strong>
                        <span class="stat-label">
                            <Files :size="13" aria-hidden="true" />
                            redundant files
                        </span>
                    </div>
                    <div class="stat accent">
                        <strong>{{ reclaimable }}</strong>
                        <span class="stat-label">
                            <HardDrive :size="13" aria-hidden="true" />
                            can be reclaimed
                        </span>
                    </div>
                </div>

                <div v-if="groups.length === 0" class="no-result">
                    <div class="no-result-icon" aria-hidden="true">
                        <Check :size="20" :stroke-width="2.5" />
                    </div>
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
                        :quick-mode="quickMode"
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
    margin-top: -10px;
    padding: 2px;
    border-radius:9px;
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
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: none;
    padding: 2px;
    border: none;
    background: transparent;
    color: inherit;
    cursor: pointer;
}

.banner-message {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    min-width: 0;
}

.banner-message :deep(svg) {
    flex: none;
    margin-top: 1px;
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

.stat-label {
    display: flex;
    align-items: center;
    gap: 5px;
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

</style>

<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { ask } from "@tauri-apps/plugin-dialog";
import {
    Check,
    Copy,
    File,
    FolderOpen,
    Hash,
    Trash,
    TriangleAlert,
} from "@lucide/vue";
import { computed, ref } from "vue";

const props = defineProps<{
    rootPath: string;
    quickMode?: boolean;
    item: {
        hash: string;
        size: string;
        size_bytes?: number;
        files: string[];
        not_duplicates?: boolean;
        sampled?: boolean;
    };
}>();
const emit = defineEmits<{ error: [message: string] }>();

type VerifyState =
    | { kind: "idle" }
    | { kind: "checking" }
    | { kind: "done"; hash: string; removed: number }
    | { kind: "no-duplicates" }
    | { kind: "error"; message: string };

const removing = ref<string | null>(null);
const verifyState = ref<VerifyState>({ kind: "idle" });

const displayHash = computed(() =>
    verifyState.value.kind === "done"
        ? verifyState.value.hash
        : props.item.hash,
);
const hashTitle = computed(() => {
    if (verifyState.value.kind === "done") {
        return `XXH64 (full): ${verifyState.value.hash}`;
    }
    return props.quickMode && props.item.sampled
        ? `XXH64 (4 KB sample): ${props.item.hash}`
        : `XXH64: ${props.item.hash}`;
});

// In quick mode, files no larger than the sample are hashed completely, so
// there is nothing left for a full-hash pass to check.
const sampleIsComplete = computed(
    () => Boolean(props.quickMode) && props.item.sampled === false,
);
const showVerify = computed(
    () =>
        !sampleIsComplete.value &&
        verifyState.value.kind !== "done" &&
        verifyState.value.kind !== "no-duplicates",
);

async function verifyGroup() {
    if (verifyState.value.kind === "checking") return;
    verifyState.value = { kind: "checking" };
    try {
        const result = await invoke<{
            hash: string | null;
            all_same: boolean;
            files: { path: string; hash: string | null; error: string | null }[];
        }>("full_hash_group", {
            rootPath: props.rootPath,
            relPaths: props.item.files,
        });

        const hashed = result.files.filter(
            (file): file is { path: string; hash: string; error: string | null } =>
                file.hash !== null,
        );
        if (hashed.length !== result.files.length) {
            const failed = result.files.find((file) => file.hash === null);
            verifyState.value = {
                kind: "error",
                message:
                    failed?.error ??
                    "Could not read every file in this group, so nothing was removed.",
            };
            return;
        }

        // Keep the largest set of files that share a full hash. Files outside
        // that set only matched on the 4 KB sample and are not duplicates.
        const byHash = new Map<string, string[]>();
        for (const file of hashed) {
            const paths = byHash.get(file.hash);
            if (paths) {
                paths.push(file.path);
            } else {
                byHash.set(file.hash, [file.path]);
            }
        }

        let bestHash = "";
        let bestPaths: string[] = [];
        for (const [hash, paths] of byHash) {
            if (paths.length > bestPaths.length) {
                bestHash = hash;
                bestPaths = paths;
            }
        }

        if (bestPaths.length < 2) {
            props.item.files.splice(0);
            props.item.not_duplicates = true;
            verifyState.value = { kind: "no-duplicates" };
            return;
        }

        const keep = new Set(bestPaths);
        const before = props.item.files.length;
        for (let index = props.item.files.length - 1; index >= 0; index--) {
            if (!keep.has(props.item.files[index])) {
                props.item.files.splice(index, 1);
            }
        }
        verifyState.value = {
            kind: "done",
            hash: bestHash,
            removed: before - props.item.files.length,
        };
    } catch (error) {
        verifyState.value = { kind: "error", message: String(error) };
    }
}

const items = computed(() =>
    props.item.files.map((full) => {
        const index = Math.max(
            full.lastIndexOf("\\"),
            full.lastIndexOf("/"),
        );
        return index === -1
            ? { full, name: full, dir: "" }
            : {
                  full,
                  name: full.slice(index + 1),
                  dir: full.slice(0, index),
              };
    }),
);

async function removeItem(path: string, parent: string[]) {
    const name = path.split(/[\\/]/).pop() ?? path;
    let confirmed = false;
    try {
        confirmed = await ask(
            `Delete "${name}" permanently?\n\nThis cannot be undone.`,
            {
                title: "Delete file",
                kind: "warning",
                okLabel: "Delete",
                cancelLabel: "Cancel",
            },
        );
    } catch (error) {
        emit("error", `Could not open the confirmation dialog: ${error}`);
        return;
    }
    if (!confirmed) return;

    removing.value = path;
    try {
        await invoke("remove_file", {
            rootPath: props.rootPath,
            relPath: path,
        });
        const index = parent.indexOf(path);
        if (index > -1) {
            parent.splice(index, 1);
        }
    } catch (error) {
        emit("error", `Could not delete "${name}": ${error}`);
    } finally {
        removing.value = null;
    }
}

async function revealItem(path: string) {
    try {
        await invoke("reveal_file_in_explorer", {
            rootPath: props.rootPath,
            relPath: path,
        });
    } catch (error) {
        emit("error", `Could not reveal the file: ${error}`);
    }
}
</script>

<template>
    <section class="dup-card">
        <header class="dup-head">
            <div class="head-left">
                <span v-if="item.files.length" class="badge">
                    <Copy :size="12" aria-hidden="true" />
                    {{ item.files.length }} copies
                </span>
                <span v-else class="badge invalid">
                    <TriangleAlert :size="12" aria-hidden="true" />
                    Not duplicates
                </span>
                <span v-if="item.files.length" class="hash" :title="hashTitle">
                    <Hash :size="12" aria-hidden="true" />
                    <span
                        v-if="
                            quickMode &&
                            item.sampled &&
                            verifyState.kind !== 'done'
                        "
                        class="hash-kind"
                    >
                        sample
                    </span>
                    <span class="hash-value">{{ displayHash }}</span>
                </span>
            </div>
            <div class="head-right">
                <span class="size">{{ item.size }}</span>
                <button
                    v-if="showVerify"
                    type="button"
                    class="verify"
                    :disabled="verifyState.kind === 'checking'"
                    :title="
                        verifyState.kind === 'checking'
                            ? 'Hashing full file contents'
                            : 'Compute full hashes for this group'
                    "
                    @click="verifyGroup"
                >
                    <Hash :size="12" aria-hidden="true" />
                    {{ verifyState.kind === "checking" ? "Hashing…" : "Full hash" }}
                </button>
            </div>
        </header>

        <p v-if="verifyState.kind === 'done'" class="verify-status match">
            <Check :size="12" aria-hidden="true" />
            <span>
                Full hash verified:
                <span class="verify-hash">{{ verifyState.hash }}</span>
                <template v-if="verifyState.removed">
                    ({{ verifyState.removed }} non-duplicate
                    {{ verifyState.removed === 1 ? "file" : "files" }} removed)
                </template>
            </span>
        </p>
        <p
            v-else-if="verifyState.kind === 'no-duplicates'"
            class="verify-status mismatch"
        >
            <TriangleAlert :size="12" aria-hidden="true" />
            <span>Full hashes differ — these files are not duplicates.</span>
        </p>
        <p v-else-if="verifyState.kind === 'error'" class="verify-status error">
            <TriangleAlert :size="12" aria-hidden="true" />
            <span>Could not compute full hashes: {{ verifyState.message }}</span>
        </p>

        <ul class="file-list">
            <li
                v-for="row in items"
                :key="row.full"
                class="file-item"
                :class="{ removing: removing === row.full }"
            >
                <File :size="18" class="file-icon" aria-hidden="true" />
                <div class="file-text" :title="row.full">
                    <span class="file-name">{{ row.name }}</span>
                    <span v-if="row.dir" class="file-dir">{{ row.dir }}</span>
                </div>
                <div class="ctrl">
                    <button
                        type="button"
                        class="reveal"
                        title="Reveal in file explorer"
                        @click="revealItem(row.full)"
                    >
                        <FolderOpen :size="13" aria-hidden="true" />
                        Reveal
                    </button>
                    <button
                        type="button"
                        class="delete"
                        title="Delete file permanently"
                        :disabled="removing !== null"
                        @click="removeItem(row.full, item.files)"
                    >
                        <Trash :size="13" aria-hidden="true" />
                        Delete
                    </button>
                </div>
            </li>
        </ul>
    </section>
</template>

<style lang="css" scoped>
.dup-card {
    padding: 14px;
    background: var(--card);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    box-shadow: var(--shadow-sm);
    content-visibility: auto;
    contain-intrinsic-size: auto 140px;
    transition:
        border-color 0.15s ease,
        box-shadow 0.15s ease;
}

.dup-card:hover {
    border-color: var(--border-strong);
    box-shadow: var(--shadow);
}

.dup-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    flex-wrap: wrap;
    padding-bottom: 10px;
    margin-bottom: 6px;
    border-bottom: 1px solid var(--border);
}

.head-left {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1 1 auto;
    min-width: 0;
}

.badge {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 2px 9px;
    border-radius: 999px;
    background: var(--accent-soft);
    color: var(--accent-deep);
    font-size: 12px;
    font-weight: 600;
    white-space: nowrap;
}

.badge.invalid {
    background: var(--danger-soft);
    color: var(--danger);
}

.head-right {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: none;
    margin-left: auto;
}

.size {
    flex: none;
    color: var(--muted);
    font-size: 12.5px;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
}

.verify {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 4px 9px;
    border-radius: 7px;
    border: 1px solid var(--border);
    background: transparent;
    color: var(--accent-deep);
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    transition:
        background 0.12s ease,
        border-color 0.12s ease,
        opacity 0.12s ease;
}

.verify:hover:not(:disabled) {
    background: var(--accent-soft);
    border-color: transparent;
}

.verify:disabled {
    opacity: 0.55;
    cursor: progress;
}

.hash {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    min-width: 0;
    overflow: hidden;
    line-height: 1;
    font-family:
        ui-monospace, SFMono-Regular, Menlo, Monaco, "Cascadia Mono",
        "Roboto Mono", monospace;
    font-size: 11px;
    color: var(--muted);
    opacity: 0.8;
}

.hash :deep(svg) {
    display: block;
    flex: none;
}

.hash-value {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    line-height: 1;
}

.hash-kind {
    flex: none;
    padding: 1px 5px;
    border-radius: 999px;
    background: var(--warning-soft);
    color: var(--warning);
    font-size: 9.5px;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
}

.verify-status {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 8px 0 0;
    padding: 7px 10px;
    border-radius: 8px;
    font-size: 12px;
    line-height: 1.4;
}

.verify-status :deep(svg) {
    flex: none;
}

.verify-status.match {
    background: var(--success-soft);
    color: var(--success);
}

.verify-status.mismatch {
    background: var(--danger-soft);
    border: 1px solid var(--danger-border);
    color: var(--danger);
}

.verify-status.error {
    background: var(--warning-soft);
    border: 1px solid var(--warning-border);
    color: var(--warning);
}

.verify-hash {
    font-family:
        ui-monospace, SFMono-Regular, Menlo, Monaco, "Cascadia Mono",
        "Roboto Mono", monospace;
    font-variant-numeric: tabular-nums;
}

.file-list {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 2px;
}

.file-item {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    border-radius: 9px;
    transition:
        background 0.12s ease,
        opacity 0.12s ease;
}

.file-icon {
    flex: none;
    color: var(--muted);
}

.file-item:hover {
    background: var(--card-subtle);
}

.file-item.removing {
    opacity: 0.45;
    pointer-events: none;
}

.file-text {
    display: flex;
    flex-direction: column;
    min-width: 0;
}

.file-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 14px;
    font-weight: 500;
    color: var(--text);
}

.file-dir {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 11.5px;
    color: var(--muted);
}

.ctrl {
    display: flex;
    gap: 6px;
    flex: none;
}

.ctrl button {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 5px 10px;
    border-radius: 7px;
    border: 1px solid var(--border);
    background: transparent;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    transition:
        background 0.12s ease,
        color 0.12s ease,
        border-color 0.12s ease,
        opacity 0.12s ease;
}

.ctrl button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
}

.ctrl button.reveal {
    color: var(--accent-deep);
}

.ctrl button.reveal:hover {
    background: var(--accent-soft);
    border-color: transparent;
}

.ctrl button.delete {
    color: var(--danger);
}

.ctrl button.delete:hover:not(:disabled) {
    background: var(--danger);
    border-color: var(--danger);
    color: #ffffff;
}

</style>

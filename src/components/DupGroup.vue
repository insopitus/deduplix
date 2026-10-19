<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { ask } from "@tauri-apps/plugin-dialog";
import { Copy, File, FolderOpen, Hash, Trash } from "@lucide/vue";
import { computed, ref } from "vue";

const props = defineProps<{
    rootPath: string;
    item: {
        hash: string;
        size: string;
        size_bytes?: number;
        files: string[];
    };
}>();
const emit = defineEmits<{ error: [message: string] }>();

const removing = ref<string | null>(null);

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
                <span class="badge">
                    <Copy :size="12" aria-hidden="true" />
                    {{ item.files.length }} copies
                </span>
                <span class="hash" :title="`XXH64: ${item.hash}`">
                    <Hash :size="12" aria-hidden="true" />
                    <span class="hash-value">{{ item.hash }}</span>
                </span>
            </div>
            <span class="size">{{ item.size }}</span>
        </header>

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

.size {
    flex: none;
    color: var(--muted);
    font-size: 12.5px;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
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

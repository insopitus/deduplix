<script setup lang="ts">
import { open } from "@tauri-apps/plugin-dialog";
import { Ref, ref } from "vue";
import LabeledInput from "./LabeledInput.vue";
import SizeUnitRadio from "./SizeUnitRadio.vue";
const emit = defineEmits(["path-selected"]);
import { SizeUnit } from "../types";
const KILO_BYTE = 1024;
const MEGA_BYTE = KILO_BYTE * 1024;
const GIGA_BYTE = MEGA_BYTE * 1024;
const include = ref("");
const exclude = ref("");
const minSize = ref(1);
const minSizeUnit: Ref<SizeUnit> = ref("KB");
const maxSize = ref(1);
const maxSizeUnit: Ref<SizeUnit> = ref("GB");

const MIN_SIZE_VALUE = 0;
const MAX_SIZE_VALUE = 999999;

async function chooseDirectory() {
    open({
        multiple: false,
        directory: true,
    })
        .then((path) => {
            console.log(path);
            if (path) {
                console.log(typeof path);
                emit("path-selected", {
                    path,
                    config: {
                        include: include.value,
                        exclude: exclude.value,
                        size_extend: [
                            convertSize(minSize.value, minSizeUnit.value),
                            convertSize(maxSize.value, maxSizeUnit.value),
                        ],
                    },
                });
            }
        })
        .catch((e) => {
            console.log(e);
        });
}
function validateSize() {
    if (minSize.value < MIN_SIZE_VALUE) {
        minSize.value = MIN_SIZE_VALUE;
    }
    if (maxSize.value > MAX_SIZE_VALUE) {
        maxSize.value = MAX_SIZE_VALUE;
    }
}
function convertSize(num: number, unit: SizeUnit) {
    let result = num;
    switch (unit) {
        case "B":
            return result;
        case "KB":
            return result * KILO_BYTE;
        case "MB":
            return result * MEGA_BYTE;
        case "GB":
            return result * GIGA_BYTE;
        default:
            return 0;
    }
}
</script>

<template>
    <div class="home">
        <div class="main">
            <div class="hero">
                <h2>Choose a directory to analyze</h2>
                <p class="muted">
                    Find duplicate files quickly — configure filters below if
                    needed.
                </p>
            </div>

            <button class="primary" @click="chooseDirectory">
                Choose a directory...
            </button>

            <LabeledInput
                v-model="include"
                label="include:"
                style="width: 100%"
                placeholder="e.g. **/*.txt, **/*.mp4 (separated by commas)"
            />
            <LabeledInput
                v-model="exclude"
                label="exclude:"
                placeholder="e.g. **/*.txt, **/*.mp4 (separated by commas)"
            />

            <div class="size-line">
                <div class="min-line">
                    <LabeledInput
                        v-model="minSize"
                        label="min size:"
                        type="number"
                        @change="validateSize"
                        placeholder="e.g. 1024"
                    />
                    <SizeUnitRadio
                        v-model="minSizeUnit"
                        name="min-size"
                        label="size unit:"
                        style="position: absolute; right: 4px; top: 37px"
                        :options="['B', 'KB', 'MB', 'GB']"
                    />
                </div>
                <div class="max-line">
                    <LabeledInput
                        v-model="maxSize"
                        label="max size:"
                        type="number"
                        @change="validateSize"
                        placeholder="e.g. 1024"
                    />

                    <SizeUnitRadio
                        v-model="maxSizeUnit"
                        name="max-size"
                        style="position: absolute; right: 4px; top: 37px"
                        :options="['B', 'KB', 'MB', 'GB']"
                    />
                </div>
            </div>
        </div>
    </div>
</template>

<style scoped>
.home {
    width: 100%;
    max-width: 740px;
}
.main {
    width: 100%;
    background: var(--card);
    border-radius: 12px;
    padding: 18px;
    box-shadow: var(--shadow);
    display: flex;
    flex-direction: column;
    gap: 12px;
}

.hero h2 {
    margin: 4px 0 2px;
    font-size: 20px;
}
.muted {
    color: var(--muted);
    margin: 0 0 8px;
    font-size: 13px;
}

button.primary {
    appearance: none;
    border: none;
    background: linear-gradient(90deg, var(--accent), var(--accent-2));
    color: white;
    padding: 10px 14px;
    font-weight: 600;
    border-radius: 10px;
    cursor: pointer;
    box-shadow: 0 6px 18px rgba(80, 120, 220, 0.12);
    transition:
        transform 0.12s ease,
        box-shadow 0.12s ease;
}
button.primary:hover {
    box-shadow: 0 10px 28px rgba(80, 120, 220, 0.16);
}

.size-line {
    display: flex;
    gap: 12px;
    flex-wrap: wrap;
}
.min-line,
.max-line {
    flex: 1 1 220px;
    position: relative;
}
</style>

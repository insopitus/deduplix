<script setup lang="ts">
import type { SizeUnit } from "../types";

defineProps<{
    options: Array<SizeUnit>;
    modelValue: SizeUnit;
    label?: string;
    embedded?: boolean;
}>();

const emit = defineEmits(["update:modelValue"]);

function updateValue(option: SizeUnit) {
    emit("update:modelValue", option);
}
</script>

<template>
    <div
        class="size-unit-radio"
        :class="{ embedded }"
        role="group"
        :aria-label="label"
    >
        <button
            v-for="option in options"
            :key="option"
            type="button"
            :class="{ selected: modelValue === option }"
            :aria-pressed="modelValue === option"
            @click="updateValue(option)"
        >
            {{ option }}
        </button>
    </div>
</template>

<style scoped>
.size-unit-radio {
    display: flex;
    align-items: stretch;
    overflow: hidden;
    border: 1px solid var(--border);
    border-radius: 9px;
    background: var(--card-subtle);
}

.size-unit-radio button {
    min-width: 38px;
    padding: 0 9px;
    font-size: 12px;
    font-weight: 500;
    border: none;
    background: transparent;
    color: var(--muted);
    cursor: pointer;
    transition:
        background 0.12s ease,
        color 0.12s ease;
}

.size-unit-radio button + button {
    border-left: 1px solid var(--border);
}

/* Used inside LabeledInput's input shell: the shell already provides the
   border and background, so the selector only adds a divider. */
.size-unit-radio.embedded {
    height: auto;
    border: none;
    border-left: 1px solid var(--border);
    border-radius: 0;
    background: transparent;
}

.size-unit-radio.embedded button {
    min-width: 34px;
    padding: 0 7px;
    font-size: 11.5px;
}

.size-unit-radio button:hover:not(.selected) {
    background: var(--accent-soft);
    color: var(--text);
}

.size-unit-radio .selected {
    background: var(--accent-fill);
    color: var(--on-accent);
    font-weight: 600;
}
</style>

<script setup lang="ts">
import { SizeUnit } from "../types";
defineProps<{
    options: Array<SizeUnit>;
    modelValue: SizeUnit;
}>();
const emit = defineEmits(["update:modelValue", "change", "blur"]);

function updateValue(option: SizeUnit) {
    emit("update:modelValue", option);
}
</script>
<template>
    <div class="size-unit-radio">
        <button
            @click="updateValue(option)"
            :class="{ selected: modelValue === option }"
            v-for="option in options"
            :key="option"
            type="button"
        >
            {{ option }}
        </button>
    </div>
</template>

<style scoped>
.size-unit-radio {
    display: flex;
    align-items: center;
    border: 1px solid rgba(2, 6, 23, 0.06);
    border-radius: 8px;
}
.size-unit-radio button {
    min-width: 36px;
    height: 28px;
    padding: 4px 8px;
    font-size: 13px;
    border: none;
    background: transparent;
    cursor: pointer;
    transition:
        background 0.12s ease,
        color 0.12s ease,
        transform 0.06s ease;
}
.size-unit-radio button:first-child {
    border-top-left-radius: 8px;
    border-bottom-left-radius: 8px;
}
.size-unit-radio button:last-child {
    border-top-right-radius: 8px;
    border-bottom-right-radius: 8px;
}
.size-unit-radio button:hover {
    /*transform: translateY(-1px);*/
}
.size-unit-radio .selected {
    background: linear-gradient(90deg, var(--accent), var(--accent-2));
    color: white;
    border-color: transparent;
    box-shadow: 0 6px 18px rgba(36, 200, 219, 0.08);
}
</style>

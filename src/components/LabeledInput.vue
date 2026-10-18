<template>
    <div class="labeled-input">
        <label :for="id">{{ label }}</label>
        <div class="input-shell">
            <input
                :id="id"
                :type="type"
                :placeholder="placeholder"
                :value="modelValue"
                :min="min"
                :max="max"
                :step="step"
                autocomplete="off"
                spellcheck="false"
                @input="updateValue"
                @change="emit('change', $event)"
                @blur="emit('blur', $event)"
            />
            <div v-if="$slots.default" class="suffix">
                <slot />
            </div>
        </div>
    </div>
</template>

<script setup lang="ts">
import { useId } from "vue";

defineProps({
    label: String,
    type: { type: String, default: "text" },
    placeholder: String,
    modelValue: [String, Number],
    min: [String, Number],
    max: [String, Number],
    step: [String, Number],
});

defineSlots<{
    default?: () => any;
}>();

const emit = defineEmits(["update:modelValue", "change", "blur"]);
const id = useId();

function updateValue(event: Event) {
    emit("update:modelValue", (event.target as HTMLInputElement).value);
}
</script>

<style lang="css" scoped>
.labeled-input {
    position: relative;
    margin-top: 10px;
    min-width: 0;
}

label {
    display: block;
    font-size: 12px;
    font-weight: 500;
    color: var(--muted);
    margin-bottom: 6px;
}

/* The shell owns the border so a slot (e.g. a unit selector) can sit
   inside the field without absolute positioning or text overlap. */
.input-shell {
    display: flex;
    align-items: stretch;
    height: 43px;
    overflow: hidden;
    border: 1px solid var(--border);
    border-radius: 9px;
    background: var(--card-subtle);
    box-shadow: 0 1px 2px rgba(2, 6, 23, 0.03);
    transition:
        border-color 0.12s ease,
        box-shadow 0.12s ease,
        background 0.12s ease;
}

.input-shell:hover {
    border-color: var(--border-strong);
}

.input-shell:focus-within {
    background: var(--card);
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-soft);
}

input {
    flex: 1 1 0;
    min-width: 0;
    padding: 10px 12px;
    border: none;
    background: transparent;
    color: var(--text);
    box-shadow: none;
}

input::placeholder {
    color: var(--muted);
    opacity: 0.75;
}

input:focus {
    outline: none;
}

.suffix {
    display: flex;
    align-items: stretch;
    flex: none;
}

input[type="number"] {
    font-variant-numeric: tabular-nums;
}
</style>

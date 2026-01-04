<template>
    <div class="labeled-input">
        <label for="label">{{ label }}</label>
        <input
            :type="type"
            :placeholder="placeholder"
            :value="modelValue"
            @input="updateValue"
            @change="emit('change', $event)"
            @blur="emit('blur', $event)"
        />
    </div>
</template>

<script setup>
const props = defineProps({
    label: String,
    type: String,
    placeholder: String,
    modelValue: [String, Number],
});

const emit = defineEmits(["update:modelValue", "change", "blur"]);

function updateValue(event) {
    emit("update:modelValue", event.target.value);
}
</script>

<style lang="css" scoped>
.labeled-input {
    position: relative;
    margin-top: 10px;
}

label {
    display: block;
    font-size: 12px;
    color: var(--muted);
    margin-bottom: 6px;
}

input {
    width: 100%;
    padding: 10px 12px;
    border-radius: 8px;
    border: 1px solid rgba(15,23,42,0.06);
    background: transparent;
    box-shadow: 0 2px 6px rgba(2,6,23,0.04);
    transition: border-color .12s ease, box-shadow .12s ease, transform .06s ease;
}
input:focus {
    outline: none;
    border-color: rgba(79,131,255,0.6);
    box-shadow: 0 8px 26px rgba(79,131,255,0.08);
    transform: translateY(-1px);
}
</style>

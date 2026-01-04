<script setup lang="ts">
import { listen } from "@tauri-apps/api/event";
import { ref } from "vue";

let stages = ["ingestion", "quick-filter", "hash"];
let stageIndex = ref(0);

listen("ingestion-done", () => {
    stageIndex.value = 1;
});
listen("quick-hash-done", () => {
    stageIndex.value = 2;
});
listen("full-hash-done", () => {
    stageIndex.value = 3;
});
</script>

<template>
    <div class="progress">
        <ul>
            <li
                v-for="[index, stage] in stages.entries()"
                class="stage"
                :class="{
                    active: stageIndex == index,
                    done: stageIndex > index,
                }"
            >
                {{ stage }}
            </li>
        </ul>
    </div>
</template>

<style scoped>
.progress ul {
    display: flex;
    width: 100%;
    padding: 0;
    margin: 0;
    gap: 8px;
}
.progress ul li {
    list-style: none;
    padding: 8px 12px;
    font-size: 13px;
    color: rgb(51, 51, 51);
    background-color: rgba(255, 255, 255, 0.6);
    flex-grow: 1;
    text-align: center;
    vertical-align: top;
    border-radius: 8px;
    border: none;
    box-shadow: 0 4px 12px rgba(2, 6, 23, 0.03);
    transition:
        transform 0.12s ease,
        background 0.12s ease;
}
.progress ul li.done {
    background: linear-gradient(
        90deg,
        rgba(79, 131, 255, 0.36),
        rgba(36, 200, 219, 0.18)
    );
}
.progress ul li.active {
    background: linear-gradient(90deg, var(--accent), var(--accent-2));
    background-size: 200% 200%;
    color: white;

    animation: gradientShift 2s linear infinite;
    transform-origin: center;
}

@keyframes gradientShift {
    0% {
        background-position: 0% 50%;
    }
    50% {
        background-position: 100% 50%;
    }
    100% {
        background-position: 0% 50%;
    }
}

@keyframes pulse {
    0% {
        box-shadow: 0 6px 18px rgba(36, 200, 219, 0.08);
        transform: translateY(0) scale(1);
    }
    50% {
        box-shadow: 0 10px 28px rgba(36, 200, 219, 0.12);
        transform: translateY(-2px) scale(1.02);
    }
    100% {
        box-shadow: 0 6px 18px rgba(36, 200, 219, 0.08);
        transform: translateY(0) scale(1);
    }
}
</style>

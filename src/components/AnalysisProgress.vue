<script setup lang="ts">
import { listen } from "@tauri-apps/api/event";
import { ref, onUnmounted } from "vue";

const stages = ["Ingesting", "Quick Filtering", "Hashing"];
const stageIndex = ref(0);
const unsubs: Array<() => void> = [];

listen("ingestion-done", () => {
    stageIndex.value = 1;
}).then((unsub) => unsubs.push(unsub));

listen("quick-hash-done", () => {
    stageIndex.value = 2;
}).then((unsub) => unsubs.push(unsub));

listen("full-hash-done", () => {
    stageIndex.value = 3;
}).then((unsub) => unsubs.push(unsub));

onUnmounted(() => {
    for (const u of unsubs) {
        try {
            u();
        } catch (e) {
            // ignore
        }
    }
});
</script>

<template>
    <div class="progress-wrap" role="status" aria-live="polite">
        <div class="bar" :class="{ running: stageIndex < stages.length }">
            <div class="bar-indicator" />
        </div>

        <div class="status-row">
            <div class="message">
                <span v-if="stageIndex < stages.length"
                    >{{ stages[stageIndex] }} — please wait</span
                >
                <span v-else>Analysis complete</span>
                <span class="dots" v-if="stageIndex < stages.length">●●●</span>
            </div>
        </div>
    </div>
</template>

<style scoped>
.progress-wrap {
    display: flex;
    flex-direction: column;
    gap: 8px;
    width: 100%;
}

/* base bar */
.bar {
    position: relative;
    width: 100%;
    height: 10px;
    background: rgba(0, 0, 0, 0.06);
    border-radius: 999px;
    overflow: hidden;
}

/* moving animated indicator (only visible when running) */
.bar .bar-indicator {
    position: absolute;
    top: 0;
    left: -40%;
    height: 100%;
    width: 40%;
    background:
        linear-gradient(
            90deg,
            rgba(255, 255, 255, 0.05),
            rgba(255, 255, 255, 0.12)
        ),
        linear-gradient(90deg, var(--accent), var(--accent-2));
    background-blend-mode: overlay;
    transform: skewX(-10deg);
}

.bar.running .bar-indicator {
    animation: slide 1.2s linear infinite;
    box-shadow: 0 6px 18px rgba(36, 200, 219, 0.08);
}

@keyframes slide {
    0% {
        left: -40%;
    }
    50% {
        left: 60%;
    }
    100% {
        left: -40%;
    }
}

/* stages row */
.status-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
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

/* done stage */
.stages li.done {
    background: linear-gradient(90deg, var(--accent), var(--accent-2));
    color: white;
    transform: translateY(-2px);
}

.message {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 160px;
    justify-content: flex-end;
    color: var(--muted);
    font-size: 13px;
}

/* animated dots to show activity */
.dots {
    display: inline-block;
    margin-left: 6px;
    letter-spacing: 4px;
    color: rgba(255, 255, 255, 0.85);
    font-weight: 700;
    text-shadow: 0 2px 6px rgba(36, 200, 219, 0.12);
    animation: dots 1s steps(3, end) infinite;
}

@keyframes dots {
    0%,
    20% {
        opacity: 0.2;
    }
    40% {
        opacity: 0.8;
    }
    60% {
        opacity: 0.2;
    }
    100% {
        opacity: 0.6;
    }
}

/* respects prefers-reduced-motion */
@media (prefers-reduced-motion: reduce) {
    .bar .bar-indicator,
    .stages li.active,
    .dots {
        animation: none !important;
    }
}
</style>

<script setup lang="ts">
import { Ref, ref } from "vue";
import SelectDir from "./components/SelectDir.vue";
import Results from "./components/Results.vue";
import { type PathConfig } from "./types";

const workingPath = ref("");
const config: Ref<PathConfig["config"]> = ref({
    size_extend: [0, 2 ** 64 - 1],
    include: "",
    exclude: "",
});
function handlePathSelect(p: PathConfig) {
    workingPath.value = p.path;
    config.value = p.config;
}
function handleGoback() {
    workingPath.value = "";
}
</script>

<template>
    <main class="container" :class="{ 'is-home': !workingPath }">
        <SelectDir
            v-if="!workingPath"
            @path-selected="handlePathSelect"
        ></SelectDir>
        <Results
            v-if="workingPath"
            :working-path="workingPath"
            :config="config"
            @back="handleGoback"
        ></Results>
    </main>
</template>

<style>
:root {
    --bg: #f4f6fb;
    --bg-glow: #e2ebff;
    --bg-end: #eef2fd;
    --bg-translucent: rgba(244, 246, 251, 0.82);
    --card: #ffffff;
    --card-subtle: #f6f8fe;
    --text: #0f172a;
    --muted: #64748b;
    --border: rgba(15, 23, 42, 0.08);
    --border-strong: rgba(15, 23, 42, 0.18);
    --accent: #4f83ff;
    --accent-2: #24c8db;
    --accent-deep: #3567e0;
    --accent-fill: #3567e0;
    --accent-fill-2: #0a7385;
    --on-accent: #ffffff;
    --accent-soft: rgba(79, 131, 255, 0.09);
    --danger: #dc2626;
    --danger-soft: rgba(220, 38, 38, 0.08);
    --danger-border: rgba(220, 38, 38, 0.24);
    --success: #16a34a;
    --success-soft: rgba(22, 163, 74, 0.1);
    --warning: #b45309;
    --warning-soft: rgba(180, 83, 9, 0.08);
    --warning-border: rgba(180, 83, 9, 0.24);
    --shadow-sm: 0 2px 8px rgba(15, 23, 42, 0.05);
    --shadow: 0 12px 32px rgba(15, 23, 42, 0.08);
    --radius: 14px;
    --gap: 12px;
    color-scheme: light;
    font-family:
        Inter, ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto,
        "Helvetica Neue", Arial, sans-serif;
    font-size: 15px;
    line-height: 1.5;
    color: var(--text);
    -webkit-font-smoothing: antialiased;
    text-rendering: optimizeLegibility;
}

html,
body,
#app {
    min-height: 100%;
}

body {
    margin: 0;
    background:
        radial-gradient(900px 480px at 50% -10%, var(--bg-glow), transparent 70%),
        linear-gradient(180deg, var(--bg) 0%, var(--bg-end) 100%);
    background-attachment: fixed;
}

* {
    box-sizing: border-box;
}

::selection {
    background: var(--accent-soft);
}

.container {
    width: 100%;
    max-width: 1040px;
    min-height: 100vh;
    margin: 0 auto;
    padding: 28px 20px 36px;
    display: flex;
    flex-direction: column;
    justify-content: flex-start;
    align-items: stretch;
    gap: var(--gap);
}

.container.is-home {
    justify-content: center;
}

/* form controls baseline */
input,
button,
textarea,
select {
    font-family: inherit;
    font-size: 14px;
    color: inherit;
}

button {
    -webkit-tap-highlight-color: transparent;
}

button:focus-visible,
a:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
}

::-webkit-scrollbar {
    width: 10px;
    height: 10px;
}
::-webkit-scrollbar-track {
    background: transparent;
}
::-webkit-scrollbar-thumb {
    background: var(--border-strong);
    background-clip: padding-box;
    border: 2px solid transparent;
    border-radius: 999px;
}
::-webkit-scrollbar-thumb:hover {
    background-color: var(--muted);
}

input[disabled] {
    color: var(--muted);
    cursor: default;
}

@media (max-width: 560px) {
    .container {
        padding: 18px 14px 28px;
    }
}

@media (prefers-color-scheme: dark) {
    :root {
        --bg: #0a101d;
        --bg-glow: #16224a;
        --bg-end: #0b1220;
        --bg-translucent: rgba(10, 16, 29, 0.82);
        --card: #111a2b;
        --card-subtle: #162134;
        --text: #e6edf7;
        --muted: #94a3b8;
        --border: rgba(148, 163, 184, 0.14);
        --border-strong: rgba(148, 163, 184, 0.3);
        --accent: #6ea8ff;
        --accent-2: #42f0e6;
        --accent-deep: #8ab6ff;
        --accent-fill: #7fb0ff;
        --accent-fill-2: #4fd8cc;
        --on-accent: #08111f;
        --accent-soft: rgba(110, 168, 255, 0.14);
        --danger: #f87171;
        --danger-soft: rgba(248, 113, 113, 0.14);
        --danger-border: rgba(248, 113, 113, 0.28);
        --success: #4ade80;
        --success-soft: rgba(74, 222, 128, 0.14);
        --warning: #fbbf24;
        --warning-soft: rgba(251, 191, 36, 0.14);
        --warning-border: rgba(251, 191, 36, 0.28);
        --shadow-sm: 0 2px 8px rgba(2, 6, 23, 0.35);
        --shadow: 0 14px 36px rgba(2, 6, 23, 0.5);
        color-scheme: dark;
    }
}

@media (prefers-reduced-motion: reduce) {
    *,
    *::before,
    *::after {
        animation-duration: 0.01ms !important;
        animation-iteration-count: 1 !important;
        transition-duration: 0.01ms !important;
    }
}
</style>

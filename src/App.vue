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
    <main class="container">
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
    --card: #ffffff;
    --muted: #6b7280;
    --accent: #4f83ff;
    --accent-2: #24c8db;
    --shadow: 0 6px 18px rgba(15, 23, 42, 0.08);
    --radius: 10px;
    --gap: 12px;
    color-scheme: light;
    font-family:
        Inter,
        ui-sans-serif,
        system-ui,
        -apple-system,
        "Segoe UI",
        Roboto,
        "Helvetica Neue",
        Arial;
    font-size: 15px;
    color: #0f172a;
}

html,
body,
#app {
    /*height: 100%;*/
    margin: 0;
    background: linear-gradient(180deg, var(--bg) 0%, #eef2fd 100%);
}

* {
    box-sizing: border-box;
}

.container {
    width: 100%;
    margin: 24px auto;
    padding: 18px;
    display: flex;
    justify-content: center;
    align-items: center;
    flex-direction: column;
    gap: var(--gap);
    min-height: calc(100vh - 48px);
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

/* small helpers */
.row {
    display: flex;
    gap: 8px;
    align-items: center;
}
.center {
    display: flex;
    justify-content: center;
    align-items: center;
}

/* make sure long file paths wrap nicely */
input[disabled] {
    color: var(--muted);
}

@media (prefers-color-scheme: dark) {
    :root {
        --bg: #0b1220;
        --card: #0f1724;
        --muted: #9aa4b2;
        --accent: #6ea8ff;
        --accent-2: #42f0e6;
        --shadow: 0 6px 18px rgba(2, 6, 23, 0.6);
        color-scheme: dark;
    }
    body {
        background: linear-gradient(180deg, #071026 0%, #071631 100%);
    }
}
</style>

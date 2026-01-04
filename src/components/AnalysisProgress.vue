<script setup lang="ts">
import { listen } from '@tauri-apps/api/event'
import { ref } from 'vue'

let stages = ['ingestion', 'quick-filter', 'hash']
let stageIndex = ref(0)

listen('ingestion-done', () => {
	stageIndex.value = 1
})
listen('quick-hash-done', () => {
	stageIndex.value = 2
})
listen('full-hash-done', () => {
	stageIndex.value = 3
})
</script>

<template>
	<div class="progress">
		<ul>
			<li v-for="[index, stage] in stages.entries()" class="stage" :class="{ active: stageIndex == index, done: stageIndex > index }">{{ stage }}</li>
		</ul>
	</div>
</template>

<style scoped>
.progress ul {
	display: flex;
	width: 100%;
	padding:0;
	margin:0;
	gap:8px;
}
.progress ul li {
	padding: 8px 12px;
	font-size: 13px;
	color: rgb(51, 51, 51);
	background-color: rgba(255,255,255,0.6);
	flex-grow: 1;
	text-align: center;
	vertical-align: top;
	border-radius: 8px;
	border: 1px solid rgba(2,6,23,0.04);
	box-shadow: 0 4px 12px rgba(2,6,23,0.03);
	transition: transform .12s ease, background .12s ease;
}
.progress ul li.active {
	background: linear-gradient(90deg, rgba(79,131,255,0.12), rgba(36,200,219,0.06));
	transform: translateY(-3px);
}
.progress ul li.done {
	background: linear-gradient(90deg,var(--accent),var(--accent-2));
	color: white;
	transform: translateY(-2px);
}
</style>

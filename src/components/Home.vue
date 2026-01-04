<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { Ref, ref } from 'vue'
import DupGroup from './DupGroup.vue'
import AnalysisProgress from './AnalysisProgress.vue'
const props = defineProps(['workingPath', 'config'])
const emit = defineEmits(['back'])
const analysisDone = ref(false)
type Pair = {
	hash: string
	files: string[]
}
const pairs: Ref<Pair[]> = ref([])
invoke<{ pairs: Pair[] }>('start_analysis', { path: props.workingPath, config: props.config }).then((result) => {
	analysisDone.value = true
	pairs.value = result.pairs
})

function goback() {
	emit('back')
}
</script>

<template>
	<header class="top-bar">
		<input :value="props.workingPath" disabled />
		<button class="back" @click="goback" :disabled="!analysisDone">Back</button>
	</header>
	<main>
		<AnalysisProgress v-if="!analysisDone"></AnalysisProgress>
		<template v-for="pair in pairs" :key="pair.hash">
			<DupGroup v-if="pair.files.length > 0" :root-path="workingPath" :item="pair"></DupGroup>
		</template>
		<div v-if="analysisDone && pairs.length === 0" class="no-result">
			No duplication found in this directory.
		</div>
	</main>
</template>

<style lang="css" scoped>
header.top-bar {
	display: flex;
	align-items: center;
	gap: 12px;
	margin-bottom: 12px;
}
header.top-bar input {
	flex-grow: 1;
	padding: 10px 12px;
	border-radius: 8px;
	border: 1px solid rgba(15,23,42,0.06);
	background: transparent;
}
button.back{
  padding: 8px 12px;
  border-radius: 8px;
  background: transparent;
  border: 1px solid rgba(15,23,42,0.06);
  cursor: pointer;
  font-weight: 600;
}
button.back:disabled{
  opacity: 0.5;
  cursor: not-allowed;
}

main {
	flex-grow: 1;
	padding-top: 6px;
	display:flex;
	flex-direction:column;
	gap: 12px;
}

.no-result {
	display: flex;
	height: 200px;
	font-size: 18px;
	align-items: center;
	justify-content: center;
	color: var(--muted);
}
</style>

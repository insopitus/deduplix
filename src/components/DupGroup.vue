<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core';

const props = defineProps(['item','rootPath'])
function removeItem(path:string,parent:String[]){
    invoke('remove_file',{rootPath:props.rootPath,relPath:path}).then(()=>{
        let index = parent.indexOf(path)
        if(index> -1){
            parent.splice(index,1)
        }
    })
}
function revealItem(path:string){
	invoke('reveal_file_in_explorer',{rootPath:props.rootPath,relPath:path})
}
</script>

<template>
	<section class="dup-card">
		<div class="dup-meta">
			<div class="hash">{{ item.hash }}</div>
			<div class="size">{{ item.size }}</div>
		</div>
		<ul class="file-list">
			<li class="file-item" v-for="child in item.files" :key="child">
				<span class="file-name" :title="child">{{ child }}</span>
				<div class="ctrl">
					<button class="reveal" @click="revealItem(child)" title="Reveal in file explorer">Reveal</button>
					<button class="delete" @click="removeItem(child,item.files)" title="Remove file">Delete</button>
				</div>
			</li>
		</ul>
	</section>
</template>

<style lang="css" scoped>
.dup-card{
  background: var(--card);
  padding: 12px;
  border-radius: 10px;
  box-shadow: var(--shadow);
  margin-bottom: 12px;
}
.dup-meta{
  display:flex;
  justify-content: space-between;
  align-items: center;
  gap: 12px;
  margin-bottom: 8px;
}
.hash{
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, 'Cascadia Mono', 'Roboto Mono', monospace;
  font-weight: 700;
  font-size: 13px;
  color: #0f172a;
  word-break: break-all;
}
.size { color: var(--muted); font-size: 13px; }

.file-list { margin:0; padding:0; list-style:none; display:flex; flex-direction:column; gap:6px; }
.file-item{
  display:flex;
  align-items:center;
  justify-content:space-between;
  padding: 8px;
  border-radius: 8px;
  transition: background .12s ease, transform .06s ease;
}
.file-item:hover{
  background: rgba(79,131,255,0.04);
}
.file-name{
  font-size: 14px;
  color: #061025;
  max-width: 75%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ctrl {
  display:flex;
  gap:8px;
}
.ctrl button{
  border: 1px solid transparent;
  background: transparent;
  padding: 6px 8px;
  border-radius: 6px;
  cursor: pointer;
  font-size: 13px;
}
.ctrl button.reveal{
  color: var(--accent);
  border: 1px solid rgba(79,131,255,0.12);
  background: rgba(79,131,255,0.04);
}
.ctrl button.reveal:hover{
  background: linear-gradient(90deg, rgba(79,131,255,0.06), rgba(36,200,219,0.04));
}
.ctrl button.delete{
  color: #b91c1c;
  border: 1px solid rgba(185,28,28,0.08);
}
.ctrl button.delete:hover{
  background: rgba(185,28,28,0.08);
  color: white;
}
</style>

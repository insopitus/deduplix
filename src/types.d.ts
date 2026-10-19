import type { Component } from "vue";

export type SizeUnit = "B" | "KB" | "MB" | "GB";
export type AnalysisStage = {
    event: string;
    label: string;
    hint: string;
    icon: Component;
};
export type StageProgress = {
    done: number;
    total: number;
};
export type PathConfig = {
    path:string,
    config:{
        include:string,
        exclude:string,
        size_extend:[number,number],
        quick_mode:boolean
    }
}
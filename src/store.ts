import { reactive } from "vue";

import type { ArrowHint } from "./arrows";

// 真实对局棋谱的一步(后端 worker 随 game_history 事件整体推送)
export interface GameStep {
    camp: string;  // 该局面当前行棋方 'w'/'b'
    notation: string; // 该步中文记谱(起始局面为空串)
    mover: string; // 走子方 'w'/'b'(起始局面为空), 供棋谱着色与框选标注
    mv: string;    // 该步 iccs 着法(如 "h2e2", 起始局面为空串), 供棋盘框选起讫格
    board: { piece: string; pos: string }[]; // 该步之后的局面
}

// 连线分析的实时状态, 供局势推演等模块读取
export const mirrorStore = reactive({
    board: {} as Record<string, string>, // 当前局面(坐标 -> 棋子, ' ' 为空)
    camp: "w", // 当前行棋方 'w'/'b'
    mirror: false, // 是否镜像视角(我方执黑)
    arrows: [] as ArrowHint[], // 招法提示箭头(优选/次优), 分析面板写入, 镜像盘渲染
    gameSteps: [] as GameStep[], // 真实对局棋谱(第 0 项为起始局面), 供局势推演从任意一步开局
});

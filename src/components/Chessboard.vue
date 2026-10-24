<script setup lang="ts">

import { emit, listen } from "@tauri-apps/api/event";
import { computed, onMounted, ref } from "vue";

import { computeArrows, cellRect } from "../arrows";
import { mirrorStore } from "../store";
import type { GameStep } from "../store";
import "../assets/css/chessboard.css";


interface Position {
    piece: string,
    pos: string,
}

interface Changed {
    piece: string,
    from: string,
    to: string,
    camp: string,
}


const startpos: Position[] = [
    { piece: "R", pos: "a0" },
    { piece: "N", pos: "b0" },
    { piece: "B", pos: "c0" },
    { piece: "A", pos: "d0" },
    { piece: "K", pos: "e0" },
    { piece: "A", pos: "f0" },
    { piece: "B", pos: "g0" },
    { piece: "N", pos: "h0" },
    { piece: "R", pos: "i0" },
    { piece: "C", pos: "b2" },
    { piece: "C", pos: "h2" },
    { piece: "P", pos: "a3" },
    { piece: "P", pos: "c3" },
    { piece: "P", pos: "e3" },
    { piece: "P", pos: "g3" },
    { piece: "P", pos: "i3" },
    { piece: "r", pos: "a9" },
    { piece: "n", pos: "b9" },
    { piece: "b", pos: "c9" },
    { piece: "a", pos: "d9" },
    { piece: "k", pos: "e9" },
    { piece: "a", pos: "f9" },
    { piece: "b", pos: "g9" },
    { piece: "n", pos: "h9" },
    { piece: "r", pos: "i9" },
    { piece: "c", pos: "b7" },
    { piece: "c", pos: "h7" },
    { piece: "p", pos: "a6" },
    { piece: "p", pos: "c6" },
    { piece: "p", pos: "e6" },
    { piece: "p", pos: "g6" },
    { piece: "p", pos: "i6" },
];

// 棋盘数据态(坐标 -> 棋子), 作为展示与预演的共同来源, DOM 仅按它渲染
const boardMap = ref<Record<string, string>>({});
let previewing = false;

const ALL_IDS: string[] = [];
for (let y = 9; y >= 0; y--) {
    for (let x = 0; x < 9; x++) {
        ALL_IDS.push(`${"abcdefghi"[x]}${y}`);
    }
}

// 招法提示箭头(优选/次优), 由分析面板写入 store, 视角翻转时随镜像重排自动重算
const arrowShapes = computed(() => computeArrows(mirrorStore.arrows, mirrorStore.mirror));

// 最新真实走子的起讫格方框标注(绿色框, 起点细落点粗; 不用箭头以免与提示箭头混淆),
// 视角翻转时随镜像重排自动重算
const moveBoxes = computed(() => {
    const steps = mirrorStore.gameSteps;
    const last = steps[steps.length - 1];
    if (!last || last.mv.length < 4) return [];
    const from = cellRect(last.mv.substring(0, 2), mirrorStore.mirror);
    const to = cellRect(last.mv.substring(2, 4), mirrorStore.mirror);
    if (!from || !to) return [];
    return [
        { ...from, color: "#18a058", width: 2.5 },
        { ...to, color: "#18a058", width: 3.5 },
    ];
});

function clearHighlights() {
    // 走子/局面重同步后旧提示箭头已失效, 直接清空
    mirrorStore.arrows = [];
}

function renderBoard(map: Record<string, string>) {
    for (const [pos, piece] of Object.entries(map)) {
        // 限定在本组件容器内查找, 避免与推演盘的同名坐标节点冲突
        const ele = document.getElementById("chessboard")?.querySelector(`[id="${pos}"]`)?.firstElementChild;
        if (!ele) continue;
        ele.classList.forEach(cls => {
            if (cls !== "piece") {
                ele.classList.remove(cls)
            }
        });
        if (piece !== " ") {
            ele.classList.add(`piece-${piece}`);
        }
    }
}

function applyPosition(pieces: Position[]) {
    const map: Record<string, string> = {};
    for (const id of ALL_IDS) map[id] = " ";
    for (const record of pieces) map[record.pos] = record.piece;
    boardMap.value = map;
    mirrorStore.board = map;
    if (!previewing) renderBoard(map);
}

function applyMove(change: Changed) {
    const map = { ...boardMap.value };
    map[change.from] = " ";
    map[change.to] = change.piece;
    boardMap.value = map;
    mirrorStore.board = map;
    if (!previewing) renderBoard(map);
}

// 主线预演: 从分析时局面的数据副本上重放着法; 预演期间实时 position/move 只更新数据不刷新 DOM
window.addEventListener("preview-line", (event) => {
    const detail = (event as CustomEvent).detail as { board: Position[]; pvs: string[]; count: number };
    if (!detail?.board?.length) return;
    const map: Record<string, string> = {};
    for (const id of ALL_IDS) map[id] = " ";
    for (const record of detail.board) map[record.pos] = record.piece;
    for (let i = 0; i < detail.count && i < detail.pvs.length; i++) {
        const from = detail.pvs[i].substring(0, 2);
        const to = detail.pvs[i].substring(2, 4);
        const piece = map[from];
        if (!piece || piece === " ") break;
        map[from] = " ";
        map[to] = piece;
    }
    previewing = true;
    renderBoard(map);
});

window.addEventListener("preview-end", () => {
    // 按最新数据态还原(预演期间可能已有实时走子)
    previewing = false;
    renderBoard(boardMap.value);
});

onMounted(async () => {
    await emit('position', startpos)
})

const wrappedItems = computed(() => {
    if (mirrorStore.mirror) {
        return [{ id: 'i0' }, { id: 'h0' }, { id: 'g0' }, { id: 'f0' }, { id: 'e0' }, { id: 'd0' }, { id: 'c0' }, { id: 'b0' }, { id: 'a0' }, { id: 'i1' }, { id: 'h1' }, { id: 'g1' }, { id: 'f1' }, { id: 'e1' }, { id: 'd1' }, { id: 'c1' }, { id: 'b1' }, { id: 'a1' }, { id: 'i2' }, { id: 'h2' }, { id: 'g2' }, { id: 'f2' }, { id: 'e2' }, { id: 'd2' }, { id: 'c2' }, { id: 'b2' }, { id: 'a2' }, { id: 'i3' }, { id: 'h3' }, { id: 'g3' }, { id: 'f3' }, { id: 'e3' }, { id: 'd3' }, { id: 'c3' }, { id: 'b3' }, { id: 'a3' }, { id: 'i4' }, { id: 'h4' }, { id: 'g4' }, { id: 'f4' }, { id: 'e4' }, { id: 'd4' }, { id: 'c4' }, { id: 'b4' }, { id: 'a4' }, { id: 'i5' }, { id: 'h5' }, { id: 'g5' }, { id: 'f5' }, { id: 'e5' }, { id: 'd5' }, { id: 'c5' }, { id: 'b5' }, { id: 'a5' }, { id: 'i6' }, { id: 'h6' }, { id: 'g6' }, { id: 'f6' }, { id: 'e6' }, { id: 'd6' }, { id: 'c6' }, { id: 'b6' }, { id: 'a6' }, { id: 'i7' }, { id: 'h7' }, { id: 'g7' }, { id: 'f7' }, { id: 'e7' }, { id: 'd7' }, { id: 'c7' }, { id: 'b7' }, { id: 'a7' }, { id: 'i8' }, { id: 'h8' }, { id: 'g8' }, { id: 'f8' }, { id: 'e8' }, { id: 'd8' }, { id: 'c8' }, { id: 'b8' }, { id: 'a8' }, { id: 'i9' }, { id: 'h9' }, { id: 'g9' }, { id: 'f9' }, { id: 'e9' }, { id: 'd9' }, { id: 'c9' }, { id: 'b9' }, { id: 'a9' }];

    } else {
        return [{ id: 'a9' }, { id: 'b9' }, { id: 'c9' }, { id: 'd9' }, { id: 'e9' }, { id: 'f9' }, { id: 'g9' }, { id: 'h9' }, { id: 'i9' }, { id: 'a8' }, { id: 'b8' }, { id: 'c8' }, { id: 'd8' }, { id: 'e8' }, { id: 'f8' }, { id: 'g8' }, { id: 'h8' }, { id: 'i8' }, { id: 'a7' }, { id: 'b7' }, { id: 'c7' }, { id: 'd7' }, { id: 'e7' }, { id: 'f7' }, { id: 'g7' }, { id: 'h7' }, { id: 'i7' }, { id: 'a6' }, { id: 'b6' }, { id: 'c6' }, { id: 'd6' }, { id: 'e6' }, { id: 'f6' }, { id: 'g6' }, { id: 'h6' }, { id: 'i6' }, { id: 'a5' }, { id: 'b5' }, { id: 'c5' }, { id: 'd5' }, { id: 'e5' }, { id: 'f5' }, { id: 'g5' }, { id: 'h5' }, { id: 'i5' }, { id: 'a4' }, { id: 'b4' }, { id: 'c4' }, { id: 'd4' }, { id: 'e4' }, { id: 'f4' }, { id: 'g4' }, { id: 'h4' }, { id: 'i4' }, { id: 'a3' }, { id: 'b3' }, { id: 'c3' }, { id: 'd3' }, { id: 'e3' }, { id: 'f3' }, { id: 'g3' }, { id: 'h3' }, { id: 'i3' }, { id: 'a2' }, { id: 'b2' }, { id: 'c2' }, { id: 'd2' }, { id: 'e2' }, { id: 'f2' }, { id: 'g2' }, { id: 'h2' }, { id: 'i2' }, { id: 'a1' }, { id: 'b1' }, { id: 'c1' }, { id: 'd1' }, { id: 'e1' }, { id: 'f1' }, { id: 'g1' }, { id: 'h1' }, { id: 'i1' }, { id: 'a0' }, { id: 'b0' }, { id: 'c0' }, { id: 'd0' }, { id: 'e0' }, { id: 'f0' }, { id: 'g0' }, { id: 'h0' }, { id: 'i0' }];
    }
});

listen('mirror', async (event) => {
    mirrorStore.mirror = event.payload as boolean;
})

listen('position', async (event) => {
    let pos = event.payload as Position[];
    clearHighlights();
    applyPosition(pos);
    // 全量重同步意味着预演已过期, 强制退出预演态
    if (previewing) {
        previewing = false;
        renderBoard(boardMap.value);
    }
})

listen('move', async (event) => {
    let change = event.payload as Changed;
    clearHighlights();
    applyMove(change);
    // 走子即换行棋方, 与局面同步更新(camp 随分析事件更新会滞后数秒, 导致推演取到旧行棋方)
    mirrorStore.camp = change.camp === "Red" ? "b" : "w";
});

// 分析事件携带权威局面, 顺带校正可能积累的显示偏差(预演中仅更新数据, 不打断预演)
listen('analyse', async (event) => {
    const data = event.payload as { board?: Position[]; camp?: string };
    if (!data?.board?.length) return;
    mirrorStore.camp = data.camp ?? mirrorStore.camp;
    applyPosition(data.board);
});

// 真实对局棋谱: 每步走子后整体推送, 供局势推演从任意一步的局面开局
listen('game_history', async (event) => {
    mirrorStore.gameSteps = event.payload as GameStep[];
});

</script>

<template>
    <div id="chessboard">
        <div v-for="(item, _) in wrappedItems" :key="item.id" :id="item.id" class="piece-wrap"><span
                class="piece"></span></div>
        <svg class="hint-arrows" viewBox="0 0 380 420">
            <!-- 最新走子的起讫格方框(先画, 箭头保持最上层) -->
            <rect v-for="(b, i) in moveBoxes" :key="'b' + i" :x="b.x + 2" :y="b.y + 2" :width="b.size - 4"
                :height="b.size - 4" rx="4" fill="none" :stroke="b.color" :stroke-width="b.width" />
            <!-- 先画全部箭杆再画全部箭头, 箭头交叉时保持头部在最上层 -->
            <line v-for="(a, i) in arrowShapes" :key="i" :x1="a.x1" :y1="a.y1" :x2="a.x2" :y2="a.y2" :stroke="a.color"
                :stroke-width="a.width" stroke-linecap="round" />
            <polygon v-for="(a, i) in arrowShapes" :key="'h' + i" :points="a.head" :fill="a.color" />
        </svg>
    </div>
</template>
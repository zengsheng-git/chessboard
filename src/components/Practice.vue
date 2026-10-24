<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { computed, nextTick, ref, watch } from "vue";
import { NButton, NCard, NFlex, NTag, NText } from "naive-ui";

import { mirrorStore } from "../store";
import { evalText, formatGap, winrateText } from "../format";
import { computeArrows, cellRect } from "../arrows";
import type { ArrowHint } from "../arrows";
import "../assets/css/chessboard.css";

interface Position {
    piece: string;
    pos: string;
}

interface PracticeState {
    board: Position[];
    camp: string;
    moves: string[];
    history: string[];
    cursor: number;      // 当前局面位于第 cursor 步之后
    notations: string[]; // 每步着法的中文记谱(与 history 等长)
    start_camp: string;  // 起始局面行棋方
}

interface MoveOutcome {
    notation: string;
    state: PracticeState;
}

const ALL_IDS: string[] = [];
for (let y = 9; y >= 0; y--) {
    for (let x = 0; x < 9; x++) {
        ALL_IDS.push(`${"abcdefghi"[x]}${y}`);
    }
}

const started = ref(false);
const tip = ref("");
const boardMap = ref<Record<string, string>>({});
const camp = ref("w");
const legalMoves = ref<string[]>([]);
const selected = ref("");
const moveTargets = ref<string[]>([]);
const thinking = ref(false);
// 当前推演会话的起点(对局棋谱第几步之后), -1=尚未开始
const practiceStartStep = ref(-1);
// 所选起点步的记谱, 用于棋谱被重建(首次识别/盘面跳变)后识别高亮是否已失效
let pickedNotation: string | null = null;
// 对局棋谱列表容器, 走子后自动滚动到最新一步
const historyListEl = ref<HTMLElement | null>(null);
const hint = ref({
    best: "----",
    score: 0,
    depth: 0,
    winrate: null as number | null,
    source: "",
    valid: false,
    camp: "w",
});
const lastLoss = ref<number | null>(null);
const pvs = ref<string[]>([]);
const followUps = ref<string[]>([]);
const alternatives = ref<string[]>([]);
const altScores = ref<number[]>([]);
const altIccs = ref<string[]>([]);
// 上一次分析的评分与行棋方, 用于计算用户走子的亏损; pendingMoves 为自上次完成分析以来的走子数
let prevScore: number | null = null;
let prevCamp = "";
let pendingMoves = 0;
let analyzeSeq = 0;

// 与镜像盘一致的视角翻转
const wrappedItems = computed(() => {
    const items: { id: string }[] = [];
    if (mirrorStore.mirror) {
        for (let y = 0; y <= 9; y++) {
            for (let x = 8; x >= 0; x--) {
                items.push({ id: `${"abcdefghi"[x]}${y}` });
            }
        }
    } else {
        for (let y = 9; y >= 0; y--) {
            for (let x = 0; x <= 8; x++) {
                items.push({ id: `${"abcdefghi"[x]}${y}` });
            }
        }
    }
    return items;
});

function cellPiece(id: string): Element | undefined | null {
    return document.getElementById("practiceboard")?.querySelector(`[id="${id}"]`)?.firstElementChild;
}

// 演练盘的招法提示箭头, 视角翻转时随镜像重排自动重算
const hintArrows = ref<ArrowHint[]>([]);
const arrowShapes = computed(() => computeArrows(hintArrows.value, mirrorStore.mirror));

// 棋谱条目悬停/选中时在演练盘上框出该步的起讫格(绿色方框, 起点细落点粗);
// 悬停离开后回落到所选起点步的着法标注, 帮助看清当前局面由哪步走来
const historyMark = ref<{ from: string; to: string } | null>(null);
const moveBoxShapes = computed(() => {
    const mark = historyMark.value;
    if (!mark) return [];
    const from = cellRect(mark.from, mirrorStore.mirror);
    const to = cellRect(mark.to, mirrorStore.mirror);
    if (!from || !to) return [];
    return [
        { ...from, color: "#18a058", width: 2.5 },
        { ...to, color: "#18a058", width: 3.5 },
    ];
});

function markOfStep(i: number): { from: string; to: string } | null {
    const step = mirrorStore.gameSteps[i];
    if (!step || step.mv.length < 4) return null;
    return { from: step.mv.substring(0, 2), to: step.mv.substring(2, 4) };
}

function hoverHistory(i: number) { historyMark.value = markOfStep(i); }

function leaveHistory() {
    historyMark.value = practiceStartStep.value >= 0 ? markOfStep(practiceStartStep.value) : null;
}

// 清除推荐/次优招法的提示箭头
function clearHintHighlight() {
    hintArrows.value = [];
}

// 在演练盘上以箭头标出最优(按行棋方红/蓝)与次优(紫)招法
function highlightHint() {
    clearHintHighlight();
    const arrows: ArrowHint[] = [];
    const pv = pvs.value[0];
    if (pv && pv.length >= 4) {
        arrows.push({ from: pv.substring(0, 2), to: pv.substring(2, 4), kind: camp.value === "w" ? "red" : "blue" });
    }
    for (const alt of altIccs.value) {
        if (alt.length >= 4) {
            arrows.push({ from: alt.substring(0, 2), to: alt.substring(2, 4), kind: "alt" });
        }
    }
    hintArrows.value = arrows;
}

function renderBoard(map: Record<string, string>) {
    for (const [pos, piece] of Object.entries(map)) {
        const ele = cellPiece(pos);
        if (!ele) continue;
        ele.classList.forEach(cls => {
            if (cls !== "piece") {
                ele.classList.remove(cls);
            }
        });
        if (piece !== " ") {
            ele.classList.add(`piece-${piece}`);
        }
    }
}

function applyState(state: PracticeState) {
    const map: Record<string, string> = {};
    for (const id of ALL_IDS) map[id] = " ";
    for (const record of state.board) map[record.pos] = record.piece;
    boardMap.value = map;
    camp.value = state.camp;
    legalMoves.value = state.moves;
    selected.value = "";
    moveTargets.value = [];
    clearHintHighlight();
    renderBoard(map);
}

function isOwnPiece(piece: string) {
    return camp.value === "w" ? piece >= "A" && piece <= "Z" : piece >= "a" && piece <= "z";
}

// 以对局棋谱第 index 步之后的局面开局推演(index=-1 表示尚未开始)
async function startAt(index: number) {
    const steps = mirrorStore.gameSteps;
    if (!steps.length) {
        tip.value = "尚未识别到对局, 请先在连线分析中启动监听";
        return;
    }
    const at = Math.max(0, Math.min(index, steps.length - 1));
    const step = steps[at];
    try {
        const state = await invoke<PracticeState>("practice_start", { board: step.board, camp: step.camp });
        lastLoss.value = null;
        prevScore = null;
        prevCamp = "";
        pendingMoves = 0;
        tip.value = "";
        applyState(state);
        started.value = true;
        practiceStartStep.value = at;
        pickedNotation = step.notation;
        // 在演练盘上框出所选起点步的起讫格, 看清起始局面由哪步走来
        historyMark.value = markOfStep(at);
        await refreshHint();
    } catch (e) {
        tip.value = String(e);
    }
}

// 从连线分析当前(最新)局面开始推演
async function startPractice() {
    await startAt(mirrorStore.gameSteps.length - 1);
}

async function onCellClick(id: string) {
    if (!started.value) return;
    if (selected.value) {
        const mv = legalMoves.value.find((m) => m === `${selected.value}${id}`);
        if (mv) {
            await doMove(selected.value, id);
            return;
        }
    }
    const piece = boardMap.value[id];
    if (piece && piece !== " " && isOwnPiece(piece)) {
        selected.value = id;
        moveTargets.value = legalMoves.value.filter((m) => m.startsWith(id)).map((m) => m.substring(2, 4));
    } else {
        selected.value = "";
        moveTargets.value = [];
    }
}

async function doMove(from: string, to: string) {
    try {
        // 思考中走子: 通知后端中断当前搜索, 引擎立即让位给新局面的分析
        const interrupt = thinking.value;
        const outcome = await invoke<MoveOutcome>("practice_do_move", { from, to, interrupt });
        applyState(outcome.state);
        // 框出刚走的这步起讫格, 与棋谱联动
        historyMark.value = { from, to };
        pendingMoves += 1;
        await refreshHint();
    } catch (e) {
        tip.value = String(e);
    }
}

// 悔棋/重置/跳转共用: 应用新局面, 复位偏离提示与亏损统计后重新分析
async function applyNavigated(state: PracticeState) {
    applyState(state);
    lastLoss.value = null;
    pendingMoves = 0;
    prevScore = null;
    prevCamp = "";
    // 悔棋/回起始后按新游标框出通往当前局面的那步着法
    historyMark.value = practiceStartStep.value >= 0 ? markOfStep(practiceStartStep.value + state.cursor) : null;
    await refreshHint();
}

async function undoMove() {
    try {
        await applyNavigated(await invoke<PracticeState>("practice_undo"));
    } catch (e) {
        tip.value = String(e);
    }
}

async function resetPractice() {
    try {
        await applyNavigated(await invoke<PracticeState>("practice_reset"));
    } catch (e) {
        tip.value = String(e);
    }
}

// 对局棋谱变化时自动滚动到最新一步, 便于随时点最新局面开局;
// 棋谱被后端整体重建(首次识别/盘面跳变)后, 原选中步骤可能已不存在, 此时复位高亮;
// 尚未开始推演时棋谱一旦可用, 自动以最新局面开始推演(打开局势推演即记录, 无需手动点击)
let autoStarting = false;
watch(() => mirrorStore.gameSteps.length, async (len) => {
    if (practiceStartStep.value >= 0
        && mirrorStore.gameSteps[practiceStartStep.value]?.notation !== pickedNotation) {
        practiceStartStep.value = -1;
        pickedNotation = null;
    }
    await nextTick();
    const el = historyListEl.value;
    if (el) el.scrollTop = el.scrollHeight;
    if (!started.value && !autoStarting && len > 0) {
        autoStarting = true;
        try {
            await startAt(len - 1);
        } finally {
            autoStarting = false;
        }
    }
});

// 棋谱条目文本: 第 0 项为起始局面, 之后为"回合. 走子方记谱 起点-落点"(如 "1. 红炮二平五 h2-e2")
function stepText(index: number): string {
    if (index === 0) return "起始";
    const step = mirrorStore.gameSteps[index];
    const side = step?.mover === "b" ? "黑" : "红";
    const mv = step?.mv ?? "";
    const coords = mv.length >= 4 ? ` ${mv.substring(0, 2)}-${mv.substring(2, 4)}` : "";
    return `${Math.ceil(index / 2)}. ${side}${step?.notation ?? ""}${coords}`;
}

// 对当前演练局面做云库优先的分析; 走子过快时旧分析的结果按序号丢弃
async function refreshHint() {
    const seq = ++analyzeSeq;
    thinking.value = true;
    hint.value = {
        best: "----",
        score: 0,
        depth: 0,
        winrate: null,
        source: "",
        valid: false,
        camp: camp.value,
    };
    try {
        const result = await invoke<any>("practice_analyze");
        if (seq !== analyzeSeq) return;
        const moves: string[] = result.moves ?? [];
        hint.value = {
            best: moves.length ? moves[0] : "无",
            score: result.score ?? 0,
            depth: result.depth ?? 0,
            winrate: result.winrate ?? null,
            source: result.source ?? "",
            valid: moves.length > 0,
            camp: result.camp ?? camp.value,
        };
        pvs.value = result.pvs ?? [];
        // 主线完整着法: 第 1 条是最优招法本身, 之后为双方接下来的推演着法
        followUps.value = moves;
        alternatives.value = result.alt_moves ?? [];
        altScores.value = result.alt_scores ?? [];
        altIccs.value = result.alternatives ?? [];
        highlightHint();
        // 亏损 = 走子前评分 + 走子后评分(均为行棋方视角), 仅在恰好走了一步时计算
        if (pendingMoves === 1 && prevScore !== null && prevCamp !== "" && prevCamp !== camp.value) {
            lastLoss.value = prevScore + result.score;
        } else {
            lastLoss.value = null;
        }
        pendingMoves = 0;
        prevScore = result.score ?? 0;
        prevCamp = camp.value;
    } catch (e) {
        if (seq !== analyzeSeq) return;
        hint.value = {
            best: "分析失败",
            score: 0,
            depth: 0,
            winrate: null,
            source: "",
            valid: false,
            camp: camp.value,
        };
        altIccs.value = [];
        clearHintHighlight();
        lastLoss.value = null;
        pendingMoves = 0;
    } finally {
        if (seq === analyzeSeq) {
            thinking.value = false;
        }
    }
}

const hintEvalText = computed(() => {
    if (thinking.value || !hint.value.valid) return "";
    const parts = [evalText(hint.value.score)];
    const wr = winrateText(hint.value.winrate, hint.value.score);
    if (wr) parts.push(wr);
    return parts.join(" · ");
});

const hintEvalType = computed(() => {
    if (thinking.value) return "warning" as const;
    if (!hint.value.valid) return "error" as const;
    if (hint.value.score > 30) return "success" as const;
    if (hint.value.score < -30) return "error" as const;
    return "info" as const;
});

const lossText = computed(() => {
    if (lastLoss.value === null) return "";
    if (lastLoss.value > 30) return `这步亏了 ${lastLoss.value} 分`;
    if (lastLoss.value < -30) return `这步赚了 ${-lastLoss.value} 分`;
    return "这步与引擎预期相当";
});
const lossType = computed(() => ((lastLoss.value ?? 0) > 30 ? "error" : "success") as "error" | "success");

// 主线着法按行棋方交替: 偶数索引是最优招法的行棋方, 奇数索引是对手
function followUpSide(index: number): string {
    const moverRed = hint.value.camp === "w";
    const isMover = index % 2 === 0;
    return (isMover ? moverRed : !moverRed) ? "红方" : "黑方";
}

// 悬停主线第 index 项: 预演到该步的局面(含最优招法); 演练盘无实时事件, 直接按数据态渲染/还原
function previewLine(index: number) {
    if (!pvs.value.length) return;
    const map = { ...boardMap.value };
    const count = Math.min(index + 1, pvs.value.length);
    for (let i = 0; i < count; i++) {
        const from = pvs.value[i].substring(0, 2);
        const to = pvs.value[i].substring(2, 4);
        const piece = map[from];
        if (!piece || piece === " ") break;
        map[from] = " ";
        map[to] = piece;
    }
    renderBoard(map);
}

function restoreBoard() {
    renderBoard(boardMap.value);
}

// 悬停次优招法: 预演走该备选首着后的局面
function previewAlt(index: number) {
    const alt = altIccs.value[index];
    if (!alt || alt.length < 4) return;
    const map = { ...boardMap.value };
    const from = alt.substring(0, 2);
    const to = alt.substring(2, 4);
    const piece = map[from];
    if (!piece || piece === " ") return;
    map[from] = " ";
    map[to] = piece;
    renderBoard(map);
}
</script>

<template>
    <div>
        <div id="practiceboard">
            <div
                v-for="(item, _) in wrappedItems"
                :key="item.id"
                :id="item.id"
                class="piece-wrap practice-cell"
                :class="{ 'sel-select': item.id === selected, 'mv-select': moveTargets.includes(item.id) }"
                @click="onCellClick(item.id)"
            >
                <span class="piece"></span>
            </div>
            <svg class="hint-arrows" viewBox="0 0 380 420">
                <!-- 棋谱联动的起讫格方框(先画, 箭头保持最上层) -->
                <rect v-for="(b, i) in moveBoxShapes" :key="'b' + i" :x="b.x + 2" :y="b.y + 2" :width="b.size - 4"
                    :height="b.size - 4" rx="4" fill="none" :stroke="b.color" :stroke-width="b.width" />
                <line v-for="(a, i) in arrowShapes" :key="i" :x1="a.x1" :y1="a.y1" :x2="a.x2" :y2="a.y2"
                    :stroke="a.color" :stroke-width="a.width" stroke-linecap="round" />
                <polygon v-for="(a, i) in arrowShapes" :key="'h' + i" :points="a.head" :fill="a.color" />
            </svg>
        </div>

        <n-card title="局势推演" :bordered="false" class="practice-panel">
            <div class="history">
                <n-text strong style="font-size: 12px">对局棋谱</n-text>
                <div ref="historyListEl" class="history-list">
                    <span
                        v-for="(step, i) in mirrorStore.gameSteps"
                        :key="i"
                        class="history-item"
                        :class="{
                            'history-start': i === 0,
                            'history-picked': practiceStartStep === i,
                            'side-red': step.mover === 'w',
                            'side-black': step.mover === 'b',
                        }"
                        :title="i === 0 ? '从起始局面开始推演' : `${stepText(i)} | 点击以这一步之后的局面开始推演`"
                        @click="startAt(i)"
                        @mouseenter="hoverHistory(i)"
                        @mouseleave="leaveHistory()"
                    >{{ stepText(i) }}</span>
                    <n-text v-if="!mirrorStore.gameSteps.length" depth="3" style="font-size: 12px">
                        暂无对局, 请先在连线分析中启动监听
                    </n-text>
                </div>
            </div>
            <div v-if="!started" class="practice-tip">
                <n-text depth="3">识别到对局后自动以最新局面开始推演; 点击棋谱任意一步可从该步的局面重开, 自由替双方试招, 每步给出提示与代价。</n-text>
                <n-button v-if="mirrorStore.gameSteps.length" type="primary" size="small" @click="startPractice">立即开始</n-button>
                <n-text v-if="tip" type="error">{{ tip }}</n-text>
            </div>
            <template v-else>
                <n-flex justify="space-between" align="end">
                    <n-text type="info" class="analyse-title" strong @mouseenter="previewLine(0)" @mouseleave="restoreBoard()">
                        {{ camp === "w" ? "红方" : "黑方" }} {{ hint.best }}
                    </n-text>
                    <n-flex :size="6" align="center">
                        <n-tag size="small" round :bordered="false" type="info">{{ hint.source }}</n-tag>
                        <n-text type="warning">深度 {{ hint.depth }}</n-text>
                    </n-flex>
                </n-flex>
                <n-flex>
                    <n-text :type="hintEvalType">{{ thinking ? "思考中…" : hintEvalText }}</n-text>
                </n-flex>
                <n-text v-if="lossText" :type="lossType" class="loss">{{ lossText }}</n-text>
                <div v-if="followUps.length" class="follow-ups">
                    <n-text depth="3">后续：</n-text>
                    <div
                        v-for="(m, i) in followUps"
                        :key="i"
                        class="follow-up-item"
                        @mouseenter="previewLine(i)"
                        @mouseleave="restoreBoard()"
                    >
                        <n-text depth="3">{{ i + 1 }}. {{ followUpSide(i) }} {{ m }}</n-text>
                    </div>
                </div>
                <div v-if="alternatives.length" class="alternatives">
                    <n-text strong style="color: #9b59b6">次优招法</n-text>
                    <div
                        v-for="(m, i) in alternatives"
                        :key="i"
                        class="alt-item"
                        @mouseenter="previewAlt(i)"
                        @mouseleave="restoreBoard()"
                    >
                        <n-text depth="3">{{ i + 2 }}. {{ m }} {{ formatGap(altScores[i]) }}</n-text>
                    </div>
                </div>
                <n-flex :size="4" class="practice-actions">
                    <n-button size="tiny" type="primary" @click="startPractice">重新推演</n-button>
                    <n-button size="tiny" @click="undoMove">悔棋</n-button>
                    <n-button size="tiny" @click="resetPractice">回起始</n-button>
                </n-flex>
            </template>
        </n-card>
    </div>
</template>

<style scoped>
.practice-panel {
    width: 260px;
    height: 440px;
    left: 1060px;
    top: 0px;
    position: absolute;
}

.practice-tip {
    display: flex;
    flex-direction: column;
    gap: 10px;
    align-items: flex-start;
}

.hint-row {
    margin-top: 6px;
}

.loss {
    margin-top: 4px;
}

/* 对局棋谱: 双列紧凑排布, 长对局纵向滚动, 点击任意一步开局推演 */
.history {
    margin-top: 2px;
}

.history-list {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 2px 4px;
    max-height: 96px;
    overflow-y: auto;
    margin-top: 4px;
    padding-right: 2px;
}

.history-item {
    font-size: 12px;
    line-height: 1.5;
    padding: 0 5px;
    border-radius: 3px;
    cursor: pointer;
    border: 1px solid rgba(128, 128, 128, 0.22);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}

.history-item.history-start {
    grid-column: 1 / -1;
}

.history-item.side-red {
    color: #d03050;
}

.history-item.side-black {
    color: #383838;
}

.history-item:hover {
    background-color: rgba(24, 160, 88, 0.15);
}

.history-item.history-picked {
    background-color: rgba(24, 160, 88, 0.25);
    font-weight: 600;
}

.follow-ups {
    margin-top: 6px;
    max-height: 92px;
    overflow-y: auto;
    font-size: 12px;
}

.follow-up-item {
    line-height: 1.7;
    border-radius: 2px;
    padding: 0 2px;
}

.follow-up-item:hover {
    background-color: rgba(24, 160, 88, 0.15);
}

.alternatives {
    margin-top: 8px;
    display: flex;
    flex-direction: column;
    gap: 2px;
}

.alt-item {
    line-height: 1.5;
    border-radius: 2px;
    padding: 0 2px;
    cursor: pointer;
}

.alt-item:hover {
    background-color: rgba(24, 160, 88, 0.15);
}

.analyse-title {
    font-size: x-large;
    cursor: pointer;
    border-radius: 2px;
    padding: 0 2px;
}

.analyse-title:hover {
    background-color: rgba(24, 160, 88, 0.15);
}

.practice-actions {
    margin-top: 8px;
}


.practice-cell {
    cursor: pointer;
}
</style>

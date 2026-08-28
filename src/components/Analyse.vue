<script setup lang="ts">
import { listen } from '@tauri-apps/api/event';
import { LogInst, NCard, NDivider, NFlex, NLog, NText } from 'naive-ui';
import { ref } from 'vue';

interface Analyse {
    depth: number,   // 深度
    score: number,   // 得分
    time: number,    // 时间
    pvs: string[],   // 思考(iccs)
    moves: string[], // 思考(chinese)
    alternatives: string[], // 次优候选首着(iccs)
    alt_moves: string[], // 次优候选(chinese)
    state: string,   // 状态
    source: string,  // 来源
    camp: string,    // 行棋方阵营 'w'/'b'
}


const logs = ref<string[]>([])
const alternatives = ref<string[]>([])
// 我方阵营，根据 mirror 事件推导（mirror=true 表示我方执黑）
const myCamp = ref<'w' | 'b'>('w')
const best = ref({
    move: "----",
    side: "--",
    depth: 0,
    evalText: "--",
    evalType: "info" as "error" | "success" | "info",
})

listen('mirror', async (event) => {
    myCamp.value = (event.payload as boolean) ? 'b' : 'w';
})

listen('analyse', async (event) => {
    let data = event.payload as Analyse;
    let mvs = data.moves.join(" ");
    logs.value.push(`<${data.source}> ${mvs}`)
    // 滚动到最新
    if (logs.value.length > 18) {
        if (logs.value.length > 128) {
            // 清理很早以前的数据
            logs.value.shift();
        }
        logInstRef.value?.scrollTo({ position: 'bottom', silent: true })
    }
    best.value.move = data.moves[0];
    best.value.depth = data.depth;
    alternatives.value = data.alt_moves ?? [];
    const isMine = data.camp === myCamp.value;
    best.value.side = isMine ? "我方" : "对方";
    const evalResult = formatEval(data.score, isMine);
    best.value.evalText = evalResult.text;
    best.value.evalType = evalResult.type;

    // 设置选择框：我方用 b-select（红色），对手用 r-select（蓝色）
    const selectClass = isMine ? "b-select" : "r-select";
    let pv = data.pvs[0];
    let from = pv.substring(0, 2);
    let to = pv.substring(2, 4)
    document.getElementById(from)?.classList.add(selectClass);
    document.getElementById(to)?.classList.add(selectClass);

    // 次优候选着法：起点与落点都标记
    for (const alt of data.alternatives ?? []) {
        if (alt.length >= 4) {
            document.getElementById(alt.substring(0, 2))?.classList.add("alt-select");
            document.getElementById(alt.substring(2, 4))?.classList.add("alt-select");
        }
    }
})


const logInstRef = ref<LogInst | null>(null)

// 分数解读：与最佳/次优/深度统一按行棋方视角，不带阵营标签（归属见标题），
// 正=行棋方占优，负=行棋方劣势；颜色按我方利益着色：红=利好我方，绿=利好对方
function formatEval(score: number, isMine: boolean): { text: string, type: "error" | "success" | "info" } {
    const abs = Math.abs(score);
    const goodForMe = isMine === (score > 0);
    const type: "error" | "success" = goodForMe ? "error" : "success";
    // 杀棋编码（行棋方视角）：正 = 行棋方杀，负 = 行棋方被杀
    if (score >= 29000) return { text: `${30000 - score}步杀`, type };
    if (score <= -30001) return { text: `${-score - 30000}步被杀`, type };
    if (score <= -29000) return { text: `${30000 + score}步被杀`, type };
    if (abs < 30) return { text: "均势", type: "info" };
    const gradePos = ["略优", "较优", "大优", "胜势"];
    const gradeNeg = ["略差", "较差", "大差", "败势"];
    const idx = abs < 150 ? 0 : abs < 400 ? 1 : abs < 800 ? 2 : 3;
    return score > 0
        ? { text: `+${abs} ${gradePos[idx]}`, type }
        : { text: `-${abs} ${gradeNeg[idx]}`, type };
}

</script>

<template>
    <n-card title="局面分析" :bordered="false" class="textlog" content-style="color: blue">
        <n-flex justify="space-between" align="end">
            <n-text type="info" class="analyse-title" strong>
                {{ best.side }} {{ best.move }}
            </n-text>
            <n-text type="warning">
                深度 {{ best.depth }}
            </n-text>
        </n-flex>
        <n-flex>
            <n-text :type="best.evalType">
                {{ best.evalText }}
            </n-text>
        </n-flex>
        <div v-if="alternatives.length" class="alternatives">
            <n-text strong style="color: #9b59b6">次优招法</n-text>
            <div v-for="(m, i) in alternatives" :key="i" class="alt-item">
                <n-text depth="3">{{ i + 2 }}. {{ m }}</n-text>
            </div>
        </div>
        <n-divider />
        <n-log class="analyse-log" :rows=18 ref="logInst" :line-height="1.5" :lines="logs" :font-size="10" />
    </n-card>
</template>

<style scoped>
.analyse-log {
    height: 230px !important;
}

.analyse-title {
    font-size: x-large;
}

.alternatives {
    margin-top: 8px;
    display: flex;
    flex-direction: column;
    gap: 2px;
}

.alt-item {
    line-height: 1.5;
}

.textlog {
    width: 260px;
    height: 440px;
    left: 400px;
    top: 0px;
}
</style>
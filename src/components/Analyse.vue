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
    state: string,   // 状态
    source: string,  // 来源
}


const logs = ref<string[]>([])
const best = ref({
    move: "----",
    depth: 0,
    evalText: "--",
    evalType: "info" as "error" | "success" | "info",
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
    const evalResult = formatEval(data.score);
    best.value.evalText = evalResult.text;
    best.value.evalType = evalResult.type;

    // 设置b-select
    let pv = data.pvs[0];
    let from = pv.substring(0, 2);
    let to = pv.substring(2, 4)
    document.getElementById(from)?.classList.add("b-select");
    document.getElementById(to)?.classList.add("b-select");
})


const logInstRef = ref<LogInst | null>(null)

// 分数解读：正分=我方(行棋方)优势；杀棋编码正分一致(30000-N)，负分不同：本地引擎 -(30000+N)、云库 -(30000-N)
function formatEval(score: number): { text: string, type: "error" | "success" | "info" } {
    if (score >= 29000) return { text: `${30000 - score}步杀`, type: "error" };
    if (score <= -30001) return { text: `${-score - 30000}步被杀`, type: "error" };
    if (score <= -29000) return { text: `${30000 + score}步被杀`, type: "error" };
    const abs = Math.abs(score);
    if (abs < 30) return { text: "均势", type: "info" };
    const grade = abs < 150 ? "略优" : abs < 400 ? "较优" : abs < 800 ? "大优" : "胜势";
    return score > 0
        ? { text: `我方${grade} +${score}`, type: "error" }
        : { text: `对方${grade} ${score}`, type: "success" };
}

</script>

<template>
    <n-card title="局面分析" :bordered="false" class="textlog" content-style="color: blue">
        <n-flex justify="space-between" align="end">
            <n-text type="info" class="analyse-title" strong>
                {{ best.move }}
            </n-text>

            <n-text :type="best.evalType">
                {{ best.evalText }}
            </n-text>
            <n-text type="warning">
                深度 {{ best.depth }}
            </n-text>
        </n-flex>
        <n-divider />
        <n-log class="analyse-log" :rows=18 ref="logInst" :line-height="1.5" :lines="logs" :font-size="10" />
    </n-card>
</template>

<style scoped>
.analyse-title {
    font-size: x-large;
}

.textlog {
    width: 260px;
    height: 440px;
    left: 400px;
    top: 0px;
}
</style>
<script setup lang="ts">
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { ref, watch } from "vue";
import { NDivider, NLayoutFooter, NDialogProvider } from "naive-ui";
import Analyse from "./components/Analyse.vue";
import Chessboard from "./components/Chessboard.vue";
import Practice from "./components/Practice.vue";
import Toolbar from "./components/Toolbar.vue";

// 顶部模式选择: LinkAnaly=连线分析, Practice=局势推演
const mode = ref("LinkAnaly");

// 推演模式加宽窗口, 观看页(镜像+分析)与推演盘并排显示
watch(mode, async (m) => {
    const width = m === "Practice" ? 1324 : 660;
    try {
        await getCurrentWindow().setSize(new LogicalSize(width, 580));
    } catch (e) {
        console.error("调整窗口尺寸失败:", e);
    }
});
</script>

<template>
    <n-dialog-provider>
        <!-- 最上面 -->
        <Toolbar v-model:mode="mode" />

        <n-divider class="spliter-toolbar" />

        <!-- 左侧: 观看页(镜像棋盘 + 分析), 推演时保持可见 -->
        <Chessboard />
        <Practice v-show="mode === 'Practice'" />

        <n-divider vertical class="spliter-middle" />
        <n-divider vertical class="spliter-right" v-show="mode === 'Practice'" />

        <!-- 右侧: 分析面板(左组) / 推演面板(右组) -->
        <Analyse />

        <n-layout-footer class="footer" position="absolute">
            <a href="https://github.com/atopx/chessboard.git">atopx: 中国象棋学习工具, 开源免费</a>
        </n-layout-footer>
    </n-dialog-provider>
</template>

<style scoped>
.footer {
    left: 13px;
    bottom: 5px;
    font-size: x-small;
}

.spliter-toolbar {
    position: absolute;
    width: 630px;
    top: 60px;
    left: 10px;
    z-index: 1;
}

.spliter-middle {
    position: absolute;
    left: 398px;
    top: 100px;
    height: 425px;
    z-index: 1;
}

.spliter-right {
    position: absolute;
    left: 1058px;
    top: 100px;
    height: 425px;
    z-index: 1;
}
</style>


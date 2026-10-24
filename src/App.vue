<script setup lang="ts">
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { onMounted, ref, watch } from "vue";
import { NDivider, NLayoutFooter, NDialogProvider } from "naive-ui";
import Analyse from "./components/Analyse.vue";
import Chessboard from "./components/Chessboard.vue";
import Practice from "./components/Practice.vue";
import Toolbar from "./components/Toolbar.vue";

// 顶部模式选择: LinkAnaly=连线分析, Practice=局势推演
const mode = ref("LinkAnaly");
// 界面缩放倍率, 持久化在本地
const zoom = ref(Number(localStorage.getItem("xqlink_zoom")) || 1);

// 按模式与缩放倍率调整 WebView 缩放和窗口尺寸, 保证内容完整显示
// 推演模式: 观看页(镜像棋盘+分析) + 棋谱列 + 推演盘 + 推演面板 并排
async function applyLayout(m: string, z: number) {
    const baseWidth = m === "Practice" ? 1512 : 660;
    try {
        await getCurrentWebview().setZoom(z);
        await getCurrentWindow().setSize(new LogicalSize(Math.round(baseWidth * z), Math.round(580 * z)));
    } catch (e) {
        console.error("调整窗口尺寸失败:", e);
    }
}

// 推演模式加宽窗口, 观看页(镜像+分析)与推演盘并排显示
watch([mode, zoom], ([m, z]) => applyLayout(m, z));

// 恢复上次的缩放设置并持久化
onMounted(() => {
    localStorage.setItem("xqlink_zoom", String(zoom.value));
    applyLayout(mode.value, zoom.value);
});
</script>

<template>
    <n-dialog-provider>
        <!-- 最上面 -->
        <Toolbar v-model:mode="mode" v-model:zoom="zoom" />

        <n-divider class="spliter-toolbar" />

        <!-- 左侧: 观看页(镜像棋盘 + 分析), 推演时保持可见 -->
        <Chessboard />
        <Practice v-show="mode === 'Practice'" :active="mode === 'Practice'" />

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
    left: 1246px;
    top: 100px;
    height: 425px;
    z-index: 1;
}
</style>


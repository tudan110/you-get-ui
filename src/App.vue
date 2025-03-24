<script setup>
import { ref, onMounted, onUnmounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open } from '@tauri-apps/plugin-dialog';
import { listen } from '@tauri-apps/api/event';

const url = ref("");
const downloadPath = ref("");
const format = ref("");
const downloading = ref(false);
const status = ref("");
const youGetInstalled = ref(false);
const cookiesPath = ref("");
const videoInfo = ref({
  title: '',
  formats: []
});
const loadingInfo = ref(false);
const downloadCaptions = ref(false);

// 格式化文件大小
const formatSize = (size) => {
  const KB = 1024;
  const MB = KB * 1024;
  const GB = MB * 1024;

  if (size >= GB) {
    return `${(size / GB).toFixed(2)} GB`;
  } else if (size >= MB) {
    return `${(size / MB).toFixed(2)} MB`;
  } else if (size >= KB) {
    return `${(size / KB).toFixed(2)} KB`;
  } else {
    return `${size} B`;
  }
};

// 格式化显示名称
const formatDisplayName = (format, quality) => {
  let displayName = format;
  if (format.includes('dash-flv')) {
    const quality = format.replace('dash-flv', '');
    switch (quality) {
      case '1080p60': displayName = '1080P60'; break;
      case '1080p': displayName = '1080P'; break;
      case '720': displayName = '720P'; break;
      case '480': displayName = '480P'; break;
      case '360': displayName = '360P'; break;
    }
  }
  
  // 如果有质量信息，添加到显示名称中
  if (quality) {
    displayName = `${displayName} (${quality})`;
  }
  
  return displayName;
};

// 监听下载进度
let unlisten = null;

async function setupProgressListener() {
  unlisten = await listen('download-progress', (event) => {
    const progress = event.payload;
    status.value = progress.message;
  });
}

async function checkYouGet() {
  try {
    youGetInstalled.value = await invoke("check_you_get_installed");
    if (!youGetInstalled.value) {
      status.value = "you-get 未安装，请点击安装按钮";
    }
  } catch (error) {
    status.value = `检查 you-get 状态失败: ${error}`;
  }
}

async function installYouGet() {
  status.value = "正在安装 you-get...";
  try {
    await invoke("install_you_get");
    youGetInstalled.value = true;
    status.value = "you-get 安装成功！";
  } catch (error) {
    status.value = `安装失败: ${error}。请手动安装 you-get。`;
    status.value += `\n官网链接: <a href="https://you-get.org/" target="_blank">https://you-get.org/</a>`;
  }
}

async function selectCookiesFile() {
  try {
    const selected = await open({
      multiple: false,
      filters: [{
        name: 'Cookies',
        extensions: ['txt']
      }]
    });
    if (selected) {
      cookiesPath.value = selected;
    }
  } catch (error) {
    status.value = `选择 cookies 文件失败: ${error}`;
  }
}

async function getVideoInfo() {
  if (!url.value) {
    status.value = "请输入视频链接";
    return;
  }

  loadingInfo.value = true;
  status.value = "正在获取视频信息...";
  
  try {
    const info = await invoke("get_video_info", { 
      url: url.value,
      cookiesPath: cookiesPath.value 
    });
    videoInfo.value = info;
    
    if (videoInfo.value.formats.length > 0) {
      format.value = videoInfo.value.formats[0].name;
      status.value = "视频信息获取成功！";
    } else {
      status.value = "未找到可用的视频格式";
    }
  } catch (error) {
    status.value = `获取视频信息失败: ${error}`;
  } finally {
    loadingInfo.value = false;
  }
}

async function startDownload() {
  if (!url.value) {
    status.value = "请输入视频链接";
    return;
  }
  
  if (!youGetInstalled.value) {
    status.value = "请先安装 you-get";
    return;
  }
  
  if (!format.value) {
    status.value = "请先获取视频信息";
    return;
  }
  
  downloading.value = true;
  status.value = "正在下载...";
  
  try {
    await invoke("download_video", {
      url: url.value,
      format: format.value,
      outputPath: downloadPath.value,
      cookiesPath: cookiesPath.value,
      noCaption: !downloadCaptions.value
    });
    status.value = "下载完成！";
  } catch (error) {
    status.value = `下载失败: ${error}`;
  } finally {
    downloading.value = false;
  }
}

async function getDefaultDownloadDir() {
  try {
    const defaultDir = await invoke("get_default_download_dir");
    downloadPath.value = defaultDir;
  } catch (error) {
    console.error('获取默认下载目录失败:', error);
  }
}

onMounted(() => {
  checkYouGet();
  getDefaultDownloadDir();
  setupProgressListener();
});

onUnmounted(() => {
  if (unlisten) {
    unlisten();
  }
});
</script>

<template>
  <main class="container">
    <div v-if="!youGetInstalled" class="install-section">
      <p>you-get 未安装，请先安装</p>
      <p>注意：you-get 依赖 ffmpeg，请确保已安装 ffmpeg。</p>
      <button @click="installYouGet" class="install-button">
        安装 you-get
      </button>

      <p class="status" v-html="status"></p>

    </div>

    <div v-else class="download-form">
      <div class="input-group">
        <div class="url-input">
          <input 
            v-model="url" 
            placeholder="请输入视频链接" 
            :disabled="downloading"
          />
          <button 
            @click="getVideoInfo" 
            :disabled="downloading || loadingInfo"
            class="info-button"
          >
            {{ loadingInfo ? '获取中...' : '获取信息' }}
          </button>
        </div>
      </div>

      <div v-if="videoInfo.title" class="video-info">
        <h3>{{ videoInfo.title }}</h3>
      </div>

      <div class="input-group">
        <input 
          v-model="downloadPath" 
          placeholder="下载路径（可选）" 
          :disabled="downloading"
        />
      </div>

      <div v-if="videoInfo.formats.length > 0" class="input-group">
        <select v-model="format" :disabled="downloading">
          <option v-for="f in videoInfo.formats" :key="f.name" :value="f.name">
            {{ formatDisplayName(f.name, f.quality) }} ({{ formatSize(f.size) }})
          </option>
        </select>
        <small class="help-text">选择视频质量，需要登录才能下载高清视频</small>
      </div>

      <div class="input-group">
        <div class="cookies-input">
          <input 
            v-model="cookiesPath" 
            placeholder="Cookies 文件路径（可选）" 
            :disabled="downloading"
            readonly
          />
          <button 
            @click="selectCookiesFile" 
            :disabled="downloading"
            class="select-button"
          >
            选择文件
          </button>
        </div>
        <small class="help-text">对于需要登录的视频（如 B 站高清视频），请选择 cookies 文件</small>
      </div>

      <div class="input-group">
        <label>
          <input type="checkbox" v-model="downloadCaptions" />
          下载字幕/弹幕
        </label>
      </div>

      <button 
        @click="startDownload" 
        :disabled="downloading || !format"
        class="download-button"
      >
        {{ downloading ? '下载中...' : '开始下载' }}
      </button>

      <div v-if="downloading" class="progress-container">
        <div class="loading-bar"></div>
        <div class="progress-text">{{ status }}</div>
      </div>

      <p v-else class="status">{{ status }}</p>
    </div>
  </main>
</template>

<style scoped>
.container {
  max-width: 800px;
  margin: 0 auto;
  padding: 2rem;
}

.install-section {
  text-align: center;
  padding: 2rem;
  background-color: #f8f9fa;
  border-radius: 8px;
  margin-bottom: 2rem;
}

.install-button {
  background-color: #007bff;
  color: white;
  padding: 1rem 2rem;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  font-size: 1rem;
  transition: background-color 0.3s;
}

.install-button:hover {
  background-color: #0056b3;
}

.download-form {
  display: flex;
  flex-direction: column;
  gap: 1rem;
  margin-top: 2rem;
}

.input-group {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  width: 100%;
}

.url-input, .cookies-input {
  display: flex;
  gap: 0.5rem;
  width: 100%;
}

.url-input input, .cookies-input input {
  flex: 1;
  width: 100%;
}

.info-button {
  background-color: #17a2b8;
  color: white;
  padding: 0.8rem 1.2rem;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  font-size: 1rem;
  transition: background-color 0.3s;
}

.info-button:hover {
  background-color: #138496;
}

.info-button:disabled {
  background-color: #cccccc;
  cursor: not-allowed;
}

.video-info {
  background-color: #f8f9fa;
  border-radius: 4px;
}

.video-info h3 {
  margin: 0;
  color: #333;
}

.select-button {
  background-color: #6c757d;
  color: white;
  padding: 0.8rem 1.2rem;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  font-size: 1rem;
  transition: background-color 0.3s;
}

.select-button:hover {
  background-color: #5a6268;
}

.select-button:disabled {
  background-color: #cccccc;
  cursor: not-allowed;
}

.help-text {
  color: #666;
  font-size: 0.875rem;
}

input, select {
  /* width: 100%; */
  padding: 0.8rem;
  border: 1px solid #ddd;
  border-radius: 4px;
  font-size: 1rem;
  box-sizing: border-box;
}

.download-button {
  background-color: #4CAF50;
  color: white;
  padding: 1rem;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  font-size: 1rem;
  transition: background-color 0.3s;
}

.download-button:hover {
  background-color: #45a049;
}

.download-button:disabled {
  background-color: #cccccc;
  cursor: not-allowed;
}

.progress-container {
  margin-top: 1rem;
  width: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 1rem;
}

.loading-bar {
  width: 100%;
  height: 4px;
  background-color: #e2e8f0;
  position: relative;
  overflow: hidden;
  border-radius: 2px;
}

.loading-bar::after {
  content: '';
  position: absolute;
  top: 0;
  left: 0;
  height: 100%;
  width: 50%;
  background-color: #3b82f6;
  animation: loading 1.5s infinite;
}

@keyframes loading {
  0% { transform: translateX(-100%); }
  100% { transform: translateX(200%); }
}

.progress-text {
  font-size: 0.875rem;
  color: #4a5568;
  text-align: center;
}

@media (prefers-color-scheme: dark) {
  .install-section,
  .video-info {
    background-color: #2f2f2f;
  }

  .video-info h3 {
    color: #f6f6f6;
  }

  input, select {
    background-color: #2f2f2f;
    color: #f6f6f6;
  }

  .status {
    color: #999;
  }

  .help-text {
    color: #999;
  }
}
</style>

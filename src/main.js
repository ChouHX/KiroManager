import { icons } from './icons.js';

const { invoke } = window.__TAURI__.core;

let accounts = [];

// ─── 注入图标 ───────────────────────────────────────────────────────────
document.getElementById('icon-download').innerHTML = icons.download;
document.getElementById('icon-folder').innerHTML = icons.folderOpen;
document.getElementById('icon-save').innerHTML = icons.save;
document.getElementById('icon-heart').innerHTML = icons.heartPulse;
document.getElementById('icon-zap').innerHTML = icons.zap;
document.querySelectorAll('.icon-refresh').forEach(el => el.innerHTML = icons.refreshCw);
document.querySelectorAll('.icon-key').forEach(el => el.innerHTML = icons.key);
document.querySelectorAll('.icon-eraser').forEach(el => el.innerHTML = icons.eraser);

// ─── Toast 通知 ─────────────────────────────────────────────────────────
function toast(msg, level = 'info') {
  const container = document.getElementById('toast-container');
  const el = document.createElement('div');
  el.className = `toast toast-${level}`;
  el.textContent = msg;
  container.appendChild(el);
  setTimeout(() => el.remove(), 3500);
}

// ─── Loading ────────────────────────────────────────────────────────────
function showLoading(text = '处理中...') {
  document.getElementById('loading-text').textContent = text;
  document.getElementById('loading-overlay').classList.remove('hidden');
}
function hideLoading() {
  document.getElementById('loading-overlay').classList.add('hidden');
}

// ─── 账号列表 ───────────────────────────────────────────────────────────
async function loadAccounts() {
  try {
    accounts = await invoke('list_accounts');
    renderCards();
  } catch (e) {
    toast('加载账号失败: ' + e, 'err');
  }
}

function renderCards() {
  const container = document.getElementById('cards-container');
  if (accounts.length === 0) {
    container.innerHTML = '<div class="text-center text-gray-500 py-16 text-sm">暂无账号，请点击上方按钮导入</div>';
    return;
  }
  container.innerHTML = `<div class="grid grid-cols-2 xl:grid-cols-3 gap-3">${accounts.map(a => cardHtml(a)).join('')}</div>`;
}

function cardHtml(a) {
  const expired = isTokenExpired(a.expires_at);
  const sub = formatSub(a.subscription);
  const usage = a.usage_limit ? `${a.current_usage}/${a.usage_limit}` : '—';

  const subClass = (sub === 'Pro+' || sub === 'Power') ? 'badge-pro-plus'
    : sub === 'Pro' ? 'badge-pro' : 'badge-free';
  const statusClass = expired ? 'badge-expired' : 'badge-ok';
  const statusText = expired ? '已过期' : '有效';
  const barClass = expired ? 'expired' : 'ok';

  return `<div class="account-card" data-id="${a.id}">
    <div class="card-status-bar ${barClass}"></div>
    <div class="p-3 pl-4">
      <div class="flex items-center gap-2 mb-2">
        <span class="text-[12px] text-gray-100 font-semibold truncate flex-1" title="${a.email}">${a.email || '—'}</span>
        <span class="badge ${subClass}">${sub}</span>
        <span class="badge ${statusClass}">${statusText}</span>
      </div>
      <div class="grid grid-cols-3 gap-x-3 gap-y-1 mb-2.5">
        <div><span class="info-label">登录</span> <span class="info-value">${a.provider || '—'}</span></div>
        <div><span class="info-label">认证</span> <span class="info-value">${a.auth_method || '—'}</span></div>
        <div><span class="info-label">用量</span> <span class="info-value">${usage}</span></div>
        <div class="col-span-2"><span class="info-label">过期</span> <span class="info-value font-mono">${a.expires_at || '—'}</span></div>
        <div><span class="info-label">区域</span> <span class="info-value">${a.region || '—'}</span></div>
      </div>
      <div class="flex items-center gap-1.5 border-t border-border pt-2">
        <button class="card-btn inline-flex items-center gap-1" onclick="refreshOne(${a.id})">${icons.refreshCw} 刷新</button>
        <button class="card-btn inline-flex items-center gap-1" onclick="injectOne(${a.id})">${icons.syringe} 注入</button>
        <button class="card-btn inline-flex items-center gap-1 text-yellow-400" onclick="enableOverageOne(${a.id})">${icons.zap} 超额</button>
        <div class="flex-1"></div>
        <button class="card-btn danger inline-flex items-center gap-1" onclick="deleteOne(${a.id})">${icons.trash2}</button>
      </div>
    </div>
  </div>`;
}

// ─── 单卡片操作 ─────────────────────────────────────────────────────────
async function refreshOne(id) {
  showLoading('刷新中...');
  try {
    const logs = await invoke('refresh_accounts', { ids: [id] });
    logs.forEach(l => toast(l, l.startsWith('✓') ? 'ok' : 'err'));
    await loadAccounts();
  } catch (e) { toast('刷新失败: ' + e, 'err'); }
  hideLoading();
}

async function injectOne(id) {
  try {
    const email = await invoke('inject_to_local', { id });
    toast(`已注入 ${email} 到本地`, 'ok');
    await reloadLocal();
  } catch (e) { toast('注入失败: ' + e, 'err'); }
}

async function deleteOne(id) {
  try {
    await invoke('delete_accounts', { ids: [id] });
    toast('已删除', 'warn');
    await loadAccounts();
  } catch (e) { toast('删除失败: ' + e, 'err'); }
}

async function enableOverageOne(id) {
  showLoading('启用超额...');
  try {
    const msg = await invoke('enable_overage_for', { id });
    toast(msg, 'ok');
    await loadAccounts();
  } catch (e) { toast('启用超额失败: ' + e, 'err'); }
  hideLoading();
}

// ─── 全局操作 ───────────────────────────────────────────────────────────
async function enableOverageAll() {
  if (accounts.length === 0) { toast('无账号', 'warn'); return; }
  showLoading(`一键超额 (${accounts.length} 个)...`);
  let ok = 0;
  for (const a of accounts) {
    try {
      const msg = await invoke('enable_overage_for', { id: a.id });
      toast(msg, 'ok');
      ok++;
    } catch (e) { toast(`${a.email}: ${e}`, 'err'); }
  }
  toast(`完成: ${ok}/${accounts.length}`, ok === accounts.length ? 'ok' : 'warn');
  await loadAccounts();
  hideLoading();
}

async function importLocal() {
  showLoading('从本地导入...');
  try {
    const email = await invoke('import_local');
    toast('已导入: ' + email, 'ok');
    await loadAccounts();
    await reloadLocal();
  } catch (e) { toast('导入失败: ' + e, 'err'); }
  hideLoading();
}

async function importJson() {
  const input = document.createElement('input');
  input.type = 'file';
  input.accept = '.json';
  input.onchange = async () => {
    const file = input.files[0];
    if (!file) return;
    showLoading('导入中...');
    const content = await file.text();
    try {
      const count = await invoke('import_json', { content });
      toast(`成功导入 ${count} 个账号`, 'ok');
      await loadAccounts();
    } catch (e) { toast('导入失败: ' + e, 'err'); }
    hideLoading();
  };
  input.click();
}

async function exportJson() {
  try {
    const json = await invoke('export_json');
    const blob = new Blob([json], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url; a.download = 'kiro_accounts_export.json'; a.click();
    URL.revokeObjectURL(url);
    toast(`已导出 ${accounts.length} 个账号`, 'ok');
  } catch (e) { toast('导出失败: ' + e, 'err'); }
}

async function healthCheck() {
  showLoading('健康检查中...');
  try {
    const logs = await invoke('health_check');
    logs.forEach(l => toast(l, 'ok'));
    await loadAccounts();
  } catch (e) { toast('健康检查失败: ' + e, 'err'); }
  hideLoading();
}

// ─── 本地状态（左侧面板）─────────────────────────────────────────────────
async function reloadLocal() {
  const container = document.getElementById('local-content');
  try {
    const info = await invoke('get_local_token');
    if (!info) {
      container.innerHTML = `
        <div class="local-status-card">
          <div class="badge badge-expired mb-2">未检测到</div>
          <p class="text-gray-500 text-[10px] leading-relaxed">本地无 Token。<br>请在右侧账号卡片点击「注入」。</p>
        </div>`;
      return;
    }
    const statusClass = info.is_expired ? 'badge-expired' : 'badge-ok';
    const statusText = info.is_expired ? '⚠ 已过期' : '✓ 有效';

    container.innerHTML = `
      <div class="local-status-card">
        <div class="flex items-center gap-2 mb-2">
          <span class="badge ${statusClass}">${statusText}</span>
          <span class="text-[10px] text-gray-500 font-mono">${info.expires_at || ''}</span>
        </div>
        <div class="space-y-1.5 text-[10px]">
          ${localField('认证', info.auth_method)}
          ${localField('登录', info.provider)}
          ${localField('区域', info.region)}
          ${localField('Hash', info.client_id_hash)}
        </div>
        <div class="mt-2 pt-2 border-t border-border/50 space-y-1">
          <div class="text-[9px] text-gray-600 uppercase tracking-wider">Tokens</div>
          <div class="text-[9px] text-gray-500 font-mono break-all leading-relaxed">${info.access_token_preview || '—'}</div>
          <div class="text-[9px] text-gray-500 font-mono break-all leading-relaxed">${info.refresh_token_preview || '—'}</div>
        </div>
      </div>`;
  } catch (e) {
    container.innerHTML = `<p class="text-red-400 text-center py-4 text-[11px]">加载失败</p>`;
  }
}

function localField(label, value) {
  return `<div class="flex items-center">
    <span class="w-8 text-gray-500 shrink-0">${label}</span>
    <span class="text-gray-300 truncate" title="${value || ''}">${value || '—'}</span>
  </div>`;
}

async function refreshLocalToken() {
  showLoading('刷新本地 Token...');
  try {
    const msg = await invoke('refresh_local_token');
    toast(msg, 'ok');
    await reloadLocal();
  } catch (e) { toast('刷新失败: ' + e, 'err'); }
  hideLoading();
}

async function clearLocalToken() {
  try {
    await invoke('clear_local_token');
    toast('已清除本地 Token', 'ok');
    await reloadLocal();
  } catch (e) { toast('清除失败: ' + e, 'err'); }
}

// ─── 工具函数 ───────────────────────────────────────────────────────────
function isTokenExpired(expiresAt) {
  if (!expiresAt) return true;
  const fmts = [/(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})/, /(\d{4})-(\d{2})-(\d{2}) (\d{2}):(\d{2}):(\d{2})/];
  for (const re of fmts) {
    const m = expiresAt.match(re);
    if (m) return new Date(m[1], m[2]-1, m[3], m[4], m[5], m[6]).getTime() < Date.now() + 5*60*1000;
  }
  return true;
}

function formatSub(raw) {
  if (!raw) return '—';
  const u = raw.toUpperCase().replace(/ /g, '_');
  if (u.includes('PRO_PLUS')) return 'Pro+';
  if (u.includes('PRO')) return 'Pro';
  if (u.includes('POWER')) return 'Power';
  if (u.includes('FREE') || u.includes('STANDALONE')) return 'Free';
  return raw || '—';
}

// ─── 自动刷新 ───────────────────────────────────────────────────────────
let autoRefreshTimer = null;
let countdownTimer = null;
let nextRefreshAt = 0;

function setAutoRefresh() {
  const mins = parseInt(document.getElementById('refresh-interval').value) || 0;
  clearInterval(autoRefreshTimer);
  clearInterval(countdownTimer);
  autoRefreshTimer = null;
  countdownTimer = null;

  if (mins <= 0) {
    document.getElementById('countdown').textContent = '--:--';
    document.getElementById('refresh-status').textContent = '';
    nextRefreshAt = 0;
    return;
  }

  nextRefreshAt = Date.now() + mins * 60 * 1000;
  autoRefreshTimer = setInterval(doAutoRefresh, mins * 60 * 1000);
  countdownTimer = setInterval(updateCountdown, 1000);
  updateCountdown();
  document.getElementById('refresh-status').textContent = `每 ${mins} 分钟自动刷新`;
}

function updateCountdown() {
  const el = document.getElementById('countdown');
  if (!nextRefreshAt) { el.textContent = '--:--'; return; }
  const diff = Math.max(0, nextRefreshAt - Date.now());
  const m = Math.floor(diff / 60000);
  const s = Math.floor((diff % 60000) / 1000);
  el.textContent = `${String(m).padStart(2,'0')}:${String(s).padStart(2,'0')}`;
  if (diff <= 0) el.textContent = '刷新中...';
}

async function doAutoRefresh() {
  const mins = parseInt(document.getElementById('refresh-interval').value) || 0;
  nextRefreshAt = Date.now() + mins * 60 * 1000;
  try {
    const msg = await invoke('refresh_all');
    toast(msg, 'ok');
    await loadAccounts();
    await reloadLocal();
  } catch (e) {
    toast('自动刷新失败: ' + e, 'err');
  }
}

// ─── 暴露全局 ───────────────────────────────────────────────────────────
Object.assign(window, {
  importLocal, importJson, exportJson, healthCheck,
  refreshOne, injectOne, deleteOne, enableOverageOne, enableOverageAll,
  reloadLocal, refreshLocalToken, clearLocalToken, setAutoRefresh
});

// ─── 初始化 ─────────────────────────────────────────────────────────────
loadAccounts();
reloadLocal();
setAutoRefresh();

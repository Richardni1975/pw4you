// ═══════════════════════════════════════════
//  pw4you v0.2 — Frontend Application Logic
//  In-Place Folder Encryption + Email Recovery
// ═══════════════════════════════════════════

const { invoke } = window.__TAURI__?.core ?? {};

// ── State ──────────────────────────────────
const state = {
  folders: [],              // Folder list items
  currentFolder: null,      // Currently unlocked folder info
  isUnlocked: false,
  activeTab: 'folders',
  config: null,             // ConfigInfo from backend
  selectedFolderPath: null, // Path selected for encryption
  orphanFolderPath: null,   // Path selected for manual (orphaned) decryption
  unlockTargetIndex: -1,    // Index in folders array for unlock modal
  forgotPasswordFolderIndex: -1,
  emailConfigured: false,
  smtpConfigured: false,
};

// ── Init ───────────────────────────────────
document.addEventListener('DOMContentLoaded', async () => {
  await loadConfig();
  await loadFolders();
  updateClock();
  setInterval(updateClock, 10000);

  // Event listeners — main buttons
  document.getElementById('btn-encrypt').addEventListener('click', handleEncryptFolder);
  document.getElementById('btn-unlock').addEventListener('click', handleUnlock);
  document.getElementById('btn-verify-code').addEventListener('click', handleVerifyCode);
  document.getElementById('btn-resend-code').addEventListener('click', handleResendCode);
  document.getElementById('btn-reset-password').addEventListener('click', handleResetPassword);
  document.getElementById('btn-bind-email').addEventListener('click', handleBindEmail);
  document.getElementById('btn-save-smtp').addEventListener('click', handleSaveSmtp);
  document.getElementById('btn-recover-v1').addEventListener('click', handleRecoverV1);

  // Orphan decrypt modal
  document.getElementById('orphan-password').addEventListener('keydown', (e) => {
    if (e.key === 'Enter') handleDecryptOrphanedConfirm();
  });
  document.getElementById('modal-orphan').addEventListener('click', (e) => {
    if (e.target === e.currentTarget) closeOrphanModal();
  });

  // Auto-lock setting change
  document.getElementById('setting-autolock').addEventListener('change', async (e) => {
    try {
      await invoke('save_config', { args: { autoLockMinutes: parseInt(e.target.value) } });
    } catch (ex) { /* ignore */ }
  });

  // Enter key handlers
  document.getElementById('modal-password-input').addEventListener('keydown', (e) => {
    if (e.key === 'Enter') handleUnlock();
  });
  document.getElementById('verification-code-input').addEventListener('keydown', (e) => {
    if (e.key === 'Enter') handleVerifyCode();
  });
  document.getElementById('encrypt-confirm').addEventListener('keydown', (e) => {
    if (e.key === 'Enter') handleEncryptFolder();
  });

  // Close modal on backdrop click
  document.getElementById('modal-password').addEventListener('click', (e) => {
    if (e.target === e.currentTarget) closePasswordModal();
  });

  // Escape to close modal
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
      closePasswordModal();
      closeOrphanModal();
    }
  });

  switchTab('folders');
  setStatus('✅ 就绪');
  toast('欢迎使用 pw4you 🔐', 'info');
});

// ── Tab Switching ──────────────────────────
function switchTab(tab) {
  state.activeTab = tab;
  document.querySelectorAll('section[id^="tab-"]').forEach(s => s.classList.add('hidden'));
  document.getElementById('tab-' + tab)?.classList.remove('hidden');

  document.querySelectorAll('.tab-btn').forEach(b => {
    b.classList.remove('active', 'bg-accent/15', 'text-accent-light');
    b.classList.add('text-gray-400');
  });
  const btn = document.querySelector(`[data-tab="${tab}"]`);
  if (btn) {
    btn.classList.add('active', 'bg-accent/15', 'text-accent-light');
    btn.classList.remove('text-gray-400');
  }
}

// ── Config ─────────────────────────────────
async function loadConfig() {
  try {
    const result = await invoke('load_config');
    if (result.ok) {
      state.config = result.data;
      state.emailConfigured = result.data.emailConfigured;
      state.smtpConfigured = result.data.smtpConfigured;

      // Populate settings fields
      if (result.data.boundEmail) {
        document.getElementById('setting-email').value = result.data.boundEmail;
        document.getElementById('setting-email-status').textContent = '✅ 已绑定安全邮箱';
        document.getElementById('setting-email-status').classList.remove('hidden');
        document.getElementById('setting-email-status').classList.add('text-success');
      }
      document.getElementById('setting-autolock').value = String(result.data.autoLockMinutes || 30);

      // Show/hide email checkbox on encrypt form
      if (result.data.emailConfigured) {
        document.getElementById('email-password-row').classList.remove('hidden');
      } else {
        document.getElementById('email-password-row').classList.add('hidden');
      }
      updateForgotPasswordButton();
    }
  } catch (e) {
    console.error('Failed to load config:', e);
  }
}

async function loadFolders() {
  try {
    const result = await invoke('list_folders');
    if (result.ok) {
      state.folders = result.data || [];
      state.isUnlocked = state.folders.some(f => f.status === 'unlocked');
      if (state.isUnlocked) {
        state.currentFolder = state.folders.find(f => f.status === 'unlocked');
      }
      renderFolderList();
      updateStatusIndicator();
    }
  } catch (e) {
    console.error('Failed to load folders:', e);
  }
}

// ── Folder List ────────────────────────────
function renderFolderList() {
  const container = document.getElementById('folder-list');

  if (state.folders.length === 0) {
    container.innerHTML = `
      <div class="text-center py-16 text-gray-500">
        <div class="text-5xl mb-4">📭</div>
        <p class="text-lg mb-2">还没有加密的文件夹</p>
        <p class="text-sm">点击「🔒 加密新文件夹」选择要加密的文件夹</p>
      </div>`;
    return;
  }

  container.innerHTML = state.folders.map((f, i) => {
    const isDecrypted = f.status === 'decrypted' || f.status === 'unlocked';
    const statusIcon = isDecrypted ? '🔓' : '🔒';
    const statusBadge = isDecrypted
      ? '<span class="tag bg-success/20 text-success mt-1">已解密</span>'
      : '<span class="tag bg-gray-500/20 text-gray-400 mt-1">已加密</span>';

    const actions = isDecrypted
      ? `<button onclick="handleReEncrypt(${i})" class="px-3 py-1.5 bg-accent text-black text-xs font-semibold rounded-xl hover:bg-accent-light transition-all">🔒 重新加密</button>`
      : `<button onclick="openPasswordModal(${i})" class="px-3 py-1.5 bg-accent text-black text-xs font-semibold rounded-xl hover:bg-accent-light transition-all">🔓 解锁</button>`;

    return `
      <div class="vault-card bg-surface-light rounded-2xl p-5 border border-white/5
                  flex items-center justify-between group">
        <div class="flex items-center gap-4 min-w-0">
          <span class="text-3xl shrink-0">${statusIcon}</span>
          <div class="min-w-0">
            <p class="font-medium text-sm truncate">${esc(f.name)}</p>
            <p class="text-xs text-gray-500 truncate max-w-[350px]" title="${esc(f.path)}">${esc(f.path)}</p>
            <div class="flex items-center gap-2">
              ${statusBadge}
              ${!isDecrypted && f.fileCount > 0 ? `<span class="text-xs text-gray-600">${f.fileCount} 个加密文件</span>` : ''}
            </div>
          </div>
        </div>
        <div class="flex items-center gap-2 opacity-0 group-hover:opacity-100 transition-opacity shrink-0">
          ${actions}
          <button onclick="handleRemoveFolder(${i})" class="px-2 py-1.5 text-gray-600 hover:text-danger text-xs transition-all" title="移除">✕</button>
        </div>
      </div>`;
  }).join('');
}

// ── Select Folder ──────────────────────────
function getDialog() {
  // Tauri v2: dialog plugin is at window.__TAURI__.dialog
  // (not window.__TAURI__.plugins.dialog)
  if (window.__TAURI__?.dialog?.open) {
    return window.__TAURI__.dialog;
  }
  if (window.__TAURI__?.plugins?.dialog?.open) {
    return window.__TAURI__.plugins.dialog;
  }
  return null;
}

async function handleSelectFolder() {
  try {
    const dialog = getDialog();
    if (dialog) {
      const path = await dialog.open({
        directory: true,
        multiple: false,
        title: '选择要加密的文件夹',
      });
      if (path) {
        state.selectedFolderPath = path;
        // Sync to manual input
        document.getElementById('encrypt-path-manual').value = path;
        const displayEl = document.getElementById('selected-folder-path');
        displayEl.textContent = '📁 已选择: ' + path;
        displayEl.classList.remove('hidden');

        // Auto-fill folder name from path
        const nameInput = document.getElementById('encrypt-name');
        if (!nameInput.value) {
          const parts = path.replace(/\\/g, '/').split('/');
          nameInput.value = parts[parts.length - 1] || '加密文件夹';
        }
      }
    } else {
      toast('文件选择器不可用，请手动输入路径', 'error');
    }
  } catch (e) {
    toast('选择文件夹失败: ' + e, 'error');
  }
}

// ── Encrypt Folder ─────────────────────────
async function handleEncryptFolder() {
  // Use browsed path first, fall back to manual input
  let path = state.selectedFolderPath;
  const manualPath = document.getElementById('encrypt-path-manual').value.trim();
  if (!path && manualPath) {
    path = manualPath;
    state.selectedFolderPath = manualPath;
  }
  const name = document.getElementById('encrypt-name').value.trim();
  const password = document.getElementById('encrypt-password').value;
  const confirm = document.getElementById('encrypt-confirm').value;
  const errDiv = document.getElementById('encrypt-error');
  const progressDiv = document.getElementById('encrypt-progress');
  const btn = document.getElementById('btn-encrypt');

  hideEncryptError();

  // Validation
  if (!path) return showEncryptError('请选择文件夹或在上方输入文件夹路径（如 F:\\myfolder）');
  if (!name) return showEncryptError('请输入文件夹名称');
  if (password.length < 6) return showEncryptError('密码至少需要 6 位');
  if (password !== confirm) return showEncryptError('两次输入的密码不一致');

  // Show progress
  btn.disabled = true;
  btn.textContent = '⏳ 正在加密...';
  progressDiv.classList.remove('hidden');
  document.getElementById('encrypt-progress-text').textContent = `正在加密 "${name}" ...`;

  try {
    const result = await invoke('encrypt_folder', {
      args: { folderPath: path, name, password }
    });

    if (result.ok) {
      state.selectedFolderPath = null;

      // Send password to email if checkbox checked
      const sendEmail = document.getElementById('chk-send-password')?.checked;
      if (sendEmail) {
        try {
          const emailResult = await invoke('send_password_to_email', {
            args: { password, folderName: name }
          });
          if (emailResult.ok) {
            toast(emailResult.data, 'success');
          } else {
            toast('⚠️ 密码邮件发送失败: ' + emailResult.error, 'error');
          }
        } catch (ex) {
          toast('⚠️ 密码邮件发送失败', 'error');
        }
      }

      // Clear form
      document.getElementById('encrypt-name').value = '';
      document.getElementById('encrypt-password').value = '';
      document.getElementById('encrypt-confirm').value = '';
      document.getElementById('encrypt-path-manual').value = '';
      document.getElementById('selected-folder-path').classList.add('hidden');
      document.getElementById('chk-send-password').checked = false;

      toast(`✅ "${name}" 加密成功！${result.data.fileCount} 个文件已加密。`, 'success');
      switchTab('folders');
      await loadFolders();
    } else {
      showEncryptError(result.error || '加密失败');
    }
  } catch (e) {
    showEncryptError('调用失败: ' + e);
  }

  btn.disabled = false;
  btn.textContent = '🔒 开始加密';
  progressDiv.classList.add('hidden');
}

function showEncryptError(msg) {
  const div = document.getElementById('encrypt-error');
  div.textContent = msg;
  div.classList.remove('hidden');
  div.classList.add('shake');
  setTimeout(() => div.classList.remove('shake'), 300);
}

function hideEncryptError() {
  document.getElementById('encrypt-error').classList.add('hidden');
}

// ── Password Modal ─────────────────────────
function openPasswordModal(index) {
  modalMode = 'unlock';
  state.unlockTargetIndex = index;
  state.forgotPasswordFolderIndex = index;
  const folder = state.folders[index];
  document.getElementById('modal-folder-name').textContent = folder.name + ' — ' + folder.path;
  document.getElementById('btn-unlock').textContent = '🔓 解锁';
  document.getElementById('modal-password-input').placeholder = '输入密码';

  // Reset views
  document.getElementById('modal-unlock-view').classList.remove('hidden');
  document.getElementById('modal-forgot-view').classList.add('hidden');
  document.getElementById('forgot-step-1').classList.remove('hidden');
  document.getElementById('forgot-step-2').classList.add('hidden');
  document.getElementById('modal-password-input').value = '';
  document.getElementById('modal-error').classList.add('hidden');
  document.getElementById('forgot-error').classList.add('hidden');

  updateForgotPasswordButton();
  document.getElementById('modal-password').classList.remove('hidden');
  document.getElementById('modal-password-input').focus();
}

function closePasswordModal() {
  document.getElementById('modal-password').classList.add('hidden');
  state.unlockTargetIndex = -1;
  state.forgotPasswordFolderIndex = -1;
  modalMode = 'unlock';
}

function updateForgotPasswordButton() {
  const btn = document.getElementById('btn-forgot-password');
  if (state.emailConfigured && state.smtpConfigured) {
    btn.classList.remove('hidden');
  } else {
    btn.classList.add('hidden');
  }
}

async function handleUnlock() {
  const password = document.getElementById('modal-password-input').value;
  if (!password) return;

  const folder = state.folders[state.unlockTargetIndex];
  if (!folder) return;

  const errDiv = document.getElementById('modal-error');
  const btn = document.getElementById('btn-unlock');
  btn.textContent = '⏳ ...';
  btn.disabled = true;
  errDiv.classList.add('hidden');

  // ── Re-encrypt mode ──
  if (modalMode === 'reencrypt') {
    if (password.length < 6) {
      errDiv.textContent = '密码至少需要 6 位';
      errDiv.classList.remove('hidden');
      btn.textContent = '🔒 重新加密';
      btn.disabled = false;
      return;
    }
    try {
      const result = await invoke('encrypt_folder', {
        args: { folderPath: folder.path, name: folder.name, password }
      });
      if (result.ok) {
        closePasswordModal();
        await loadFolders();
        toast(`✅ "${folder.name}" 已重新加密 — ${result.data.fileCount} 个文件`, 'success');
      } else {
        errDiv.textContent = result.error || '加密失败';
        errDiv.classList.remove('hidden');
      }
    } catch (e) {
      errDiv.textContent = '调用失败: ' + e;
      errDiv.classList.remove('hidden');
    }
    btn.textContent = '🔒 重新加密';
    btn.disabled = false;
    return;
  }

  // ── Normal unlock mode ──
  try {
    const result = await invoke('unlock_folder', {
      args: { folderPath: folder.path, password }
    });

    if (result.ok && result.data.status === 'success') {
      closePasswordModal();
      state.isUnlocked = false;
      state.currentFolder = null;
      await loadFolders();
      toast(result.data.message + ` (${result.data.filesDecrypted} 个文件)`, 'success');
    } else if (result.ok) {
      errDiv.textContent = result.data.message;
      errDiv.classList.remove('hidden');
      errDiv.classList.add('shake');
      setTimeout(() => errDiv.classList.remove('shake'), 300);
    } else {
      errDiv.textContent = result.error || '解锁失败';
      errDiv.classList.remove('hidden');
    }
  } catch (e) {
    errDiv.textContent = '调用失败: ' + e;
    errDiv.classList.remove('hidden');
  }

  btn.textContent = '🔓 解锁';
  btn.disabled = false;
}

// ── Re-Encrypt (modal mode toggle) ────────
// modalMode: 'unlock' = normal unlock+decrypt, 'reencrypt' = encrypt a decrypted folder
let modalMode = 'unlock';

function openReEncryptModal(index) {
  modalMode = 'reencrypt';
  state.unlockTargetIndex = index;
  const folder = state.folders[index];
  document.getElementById('modal-folder-name').textContent = '🔒 重新加密: ' + folder.name;
  document.getElementById('modal-unlock-view').classList.remove('hidden');
  document.getElementById('modal-forgot-view').classList.add('hidden');
  document.getElementById('modal-password-input').value = '';
  document.getElementById('modal-error').classList.add('hidden');
  document.getElementById('btn-forgot-password').classList.add('hidden');
  document.getElementById('btn-unlock').textContent = '🔒 重新加密';
  document.getElementById('modal-password').classList.remove('hidden');
  document.getElementById('modal-password-input').placeholder = '输入加密密码（至少6位）';
  document.getElementById('modal-password-input').focus();
}

async function handleReEncrypt(index) {
  openReEncryptModal(index);
}

// ── Remove Folder ──────────────────────────
async function handleRemoveFolder(index) {
  const folder = state.folders[index];
  if (!confirm(`确定要从列表中移除 "${folder.name}"？\n\n⚠️ 加密文件不会被删除，只是移除注册信息。\n你仍然需要密码才能解密该文件夹。`)) return;

  try {
    const result = await invoke('remove_folder', { args: { folderPath: folder.path } });
    if (result.ok) {
      await loadFolders();
      toast(result.data || '已移除', 'info');
    } else {
      toast(result.error || '移除失败', 'error');
    }
  } catch (e) {
    toast('移除失败: ' + e, 'error');
  }
}

// ── Forgot Password Flow ───────────────────
async function showForgotPassword() {
  if (!state.emailConfigured) {
    toast('❌ 未绑定安全邮箱。请先在「设置」中绑定邮箱。', 'error');
    return;
  }
  if (!state.smtpConfigured) {
    toast('❌ 未配置邮件服务器。请先在「设置」中配置 SMTP 设置。', 'error');
    return;
  }

  document.getElementById('modal-unlock-view').classList.add('hidden');
  document.getElementById('modal-forgot-view').classList.remove('hidden');
  document.getElementById('forgot-step-1').classList.remove('hidden');
  document.getElementById('forgot-step-2').classList.add('hidden');
  document.getElementById('forgot-error').classList.add('hidden');
  document.getElementById('verification-code-input').value = '';

  // Send verification code
  try {
    const result = await invoke('send_verification_code');
    if (result.ok) {
      document.getElementById('forgot-email-display').textContent = result.data.maskedEmail;
      document.getElementById('modal-email-hint').textContent = result.data.message;
      toast('📧 验证码已发送', 'info');
    } else {
      document.getElementById('forgot-error').textContent = result.error || '发送失败';
      document.getElementById('forgot-error').classList.remove('hidden');
    }
  } catch (e) {
    document.getElementById('forgot-error').textContent = '发送失败: ' + e;
    document.getElementById('forgot-error').classList.remove('hidden');
  }

  document.getElementById('verification-code-input').focus();
}

function backToUnlockView() {
  document.getElementById('modal-forgot-view').classList.add('hidden');
  document.getElementById('modal-unlock-view').classList.remove('hidden');
  document.getElementById('modal-error').classList.add('hidden');
  document.getElementById('modal-password-input').focus();
}

async function handleVerifyCode() {
  const code = document.getElementById('verification-code-input').value.trim();
  if (code.length !== 6) {
    document.getElementById('forgot-error').textContent = '请输入 6 位验证码';
    document.getElementById('forgot-error').classList.remove('hidden');
    return;
  }

  const folder = state.folders[state.forgotPasswordFolderIndex];
  if (!folder) return;

  const btn = document.getElementById('btn-verify-code');
  btn.textContent = '⏳ ...';
  btn.disabled = true;
  document.getElementById('forgot-error').classList.add('hidden');

  try {
    const result = await invoke('verify_code_and_reset', {
      args: { code, folderPath: folder.path, newPassword: null }
    });

    if (result.ok && result.data.status === 'verified') {
      // Code verified — show step 2 (set new password)
      document.getElementById('forgot-step-1').classList.add('hidden');
      document.getElementById('forgot-step-2').classList.remove('hidden');
      document.getElementById('new-password-input').value = '';
      document.getElementById('new-password-confirm').value = '';
      document.getElementById('reset-error').classList.add('hidden');
      document.getElementById('new-password-input').focus();
    } else {
      document.getElementById('forgot-error').textContent = result.error || '验证失败';
      document.getElementById('forgot-error').classList.remove('hidden');
    }
  } catch (e) {
    document.getElementById('forgot-error').textContent = '验证失败: ' + e;
    document.getElementById('forgot-error').classList.remove('hidden');
  }

  btn.textContent = '✅ 验证';
  btn.disabled = false;
}

async function handleResendCode() {
  const btn = document.getElementById('btn-resend-code');
  btn.textContent = '⏳ 正在发送...';
  btn.disabled = true;
  document.getElementById('forgot-error').classList.add('hidden');

  try {
    const result = await invoke('send_verification_code');
    if (result.ok) {
      toast('📧 验证码已重新发送', 'success');
      document.getElementById('verification-code-input').value = '';
      document.getElementById('verification-code-input').focus();
    } else {
      document.getElementById('forgot-error').textContent = result.error || '发送失败';
      document.getElementById('forgot-error').classList.remove('hidden');
    }
  } catch (e) {
    document.getElementById('forgot-error').textContent = '发送失败: ' + e;
    document.getElementById('forgot-error').classList.remove('hidden');
  }

  btn.textContent = '📤 重新发送验证码';
  btn.disabled = false;
}

async function handleResetPassword() {
  const newPassword = document.getElementById('new-password-input').value;
  const confirm = document.getElementById('new-password-confirm').value;
  const errDiv = document.getElementById('reset-error');

  if (newPassword.length < 6) {
    errDiv.textContent = '新密码至少需要 6 位';
    errDiv.classList.remove('hidden');
    return;
  }
  if (newPassword !== confirm) {
    errDiv.textContent = '两次输入的密码不一致';
    errDiv.classList.remove('hidden');
    return;
  }

  const folder = state.folders[state.forgotPasswordFolderIndex];
  if (!folder) return;

  const btn = document.getElementById('btn-reset-password');
  btn.textContent = '⏳ ...';
  btn.disabled = true;
  errDiv.classList.add('hidden');

  try {
    const result = await invoke('verify_code_and_reset', {
      args: { code: 'used', folderPath: folder.path, newPassword }
    });

    if (result.ok) {
      closePasswordModal();
      toast('✅ 密码已重置！文件夹已解锁。', 'success');
      state.isUnlocked = true;
      state.currentFolder = folder;
      await loadFolders();
    } else {
      errDiv.textContent = result.error || '密码重置失败';
      errDiv.classList.remove('hidden');
    }
  } catch (e) {
    errDiv.textContent = '重置失败: ' + e;
    errDiv.classList.remove('hidden');
  }

  btn.textContent = '🔑 重置密码';
  btn.disabled = false;
}

// ── Settings Handlers ──────────────────────
async function handleBindEmail() {
  const email = document.getElementById('setting-email').value.trim();
  const statusEl = document.getElementById('setting-email-status');

  if (!email || !email.includes('@')) {
    statusEl.textContent = '请输入有效的邮箱地址';
    statusEl.classList.remove('hidden', 'text-success');
    statusEl.classList.add('text-danger');
    return;
  }

  const btn = document.getElementById('btn-bind-email');
  btn.textContent = '⏳';
  btn.disabled = true;

  try {
    const result = await invoke('bind_email', { args: { email } });
    if (result.ok) {
      statusEl.textContent = '✅ ' + result.data;
      statusEl.classList.remove('hidden', 'text-danger');
      statusEl.classList.add('text-success');
      state.emailConfigured = true;
      await loadConfig();
      toast('📧 安全邮箱绑定成功', 'success');
    } else {
      statusEl.textContent = result.error || '绑定失败';
      statusEl.classList.remove('hidden', 'text-success');
      statusEl.classList.add('text-danger');
    }
  } catch (e) {
    statusEl.textContent = '绑定失败: ' + e;
    statusEl.classList.remove('hidden', 'text-success');
    statusEl.classList.add('text-danger');
  }

  btn.textContent = '绑定';
  btn.disabled = false;
}

async function handleSaveSmtp() {
  const server = document.getElementById('smtp-server').value.trim();
  const port = parseInt(document.getElementById('smtp-port').value) || 587;
  const username = document.getElementById('smtp-username').value.trim();
  const password = document.getElementById('smtp-password').value;
  const statusEl = document.getElementById('smtp-status');

  if (!server || !username) {
    statusEl.textContent = '请填写 SMTP 服务器地址和发件邮箱';
    statusEl.classList.remove('hidden', 'text-success');
    statusEl.classList.add('text-danger');
    return;
  }

  const btn = document.getElementById('btn-save-smtp');
  btn.textContent = '⏳';
  btn.disabled = true;

  try {
    const result = await invoke('update_smtp_config', {
      args: { server, port, username, password }
    });
    if (result.ok) {
      statusEl.textContent = '✅ ' + result.data;
      statusEl.classList.remove('hidden', 'text-danger');
      statusEl.classList.add('text-success');
      state.smtpConfigured = true;
      await loadConfig();
      toast('📨 邮件设置已保存', 'success');
    } else {
      statusEl.textContent = result.error || '保存失败';
      statusEl.classList.remove('hidden', 'text-success');
      statusEl.classList.add('text-danger');
    }
  } catch (e) {
    statusEl.textContent = '保存失败: ' + e;
    statusEl.classList.remove('hidden', 'text-success');
    statusEl.classList.add('text-danger');
  }

  btn.textContent = '💾 保存邮件设置';
  btn.disabled = false;
}

// ── Status Indicator ───────────────────────
function updateStatusIndicator() {
  const indicator = document.getElementById('status-indicator');
  if (state.isUnlocked && state.currentFolder) {
    indicator.innerHTML = `
      <span class="w-2 h-2 rounded-full bg-green-400 unlock-indicator"></span>
      <span class="text-green-400 text-xs">${esc(state.currentFolder.name)}</span>`;
  } else {
    indicator.innerHTML = `
      <span class="w-2 h-2 rounded-full bg-red-400"></span>
      <span class="text-gray-400 text-xs">未解锁</span>`;
  }
}

// ── Clock ──────────────────────────────────
function updateClock() {
  const now = new Date();
  document.getElementById('footer-time').textContent =
    now.toLocaleString('zh-CN', { hour: '2-digit', minute: '2-digit', second: '2-digit' });
}

// ── Toast ──────────────────────────────────
function toast(msg, type = 'info') {
  const container = document.getElementById('toast-container');
  const colors = {
    success: 'bg-success/20 border-success/30 text-success',
    error: 'bg-danger/20 border-danger/30 text-danger',
    info: 'bg-surface-light border-white/10 text-gray-200',
  };

  const el = document.createElement('div');
  el.className = `toast px-4 py-2.5 rounded-xl border text-sm shadow-xl ${colors[type] || colors.info}`;
  el.textContent = msg;
  container.appendChild(el);

  setTimeout(() => el.remove(), 4000);
}

// ── Footer Status ──────────────────────────
function setStatus(msg) {
  document.getElementById('footer-status').textContent = msg;
}

// ── V1 Recovery ────────────────────────────
async function handleRecoverV1() {
  const password = document.getElementById('recover-v1-password').value;
  if (!password) {
    document.getElementById('recover-v1-status').textContent = '请输入旧版密码';
    document.getElementById('recover-v1-status').classList.remove('hidden');
    return;
  }

  // Select folder
  const dialog = getDialog();
  if (!dialog) {
    document.getElementById('recover-v1-status').textContent = '文件选择器不可用，请使用手动输入';
    document.getElementById('recover-v1-status').classList.remove('hidden');
    return;
  }

  const path = await dialog.open({
    directory: true,
    multiple: false,
    title: '选择要恢复的旧版加密文件夹（包含 .pw4lock 和 .pw4e 文件）',
  });

  if (!path) return;

  const btn = document.getElementById('btn-recover-v1');
  btn.textContent = '⏳ 恢复中...';
  btn.disabled = true;
  document.getElementById('recover-v1-status').classList.add('hidden');

  try {
    const result = await invoke('recover_v1_folder', {
      args: { folderPath: path, masterPassword: password }
    });
    if (result.ok) {
      document.getElementById('recover-v1-status').textContent = result.data;
      document.getElementById('recover-v1-status').classList.remove('hidden');
      document.getElementById('recover-v1-status').classList.add('text-success');
      toast(result.data, 'success');
    } else {
      document.getElementById('recover-v1-status').textContent = result.error;
      document.getElementById('recover-v1-status').classList.remove('hidden');
      document.getElementById('recover-v1-status').classList.add('text-danger');
    }
  } catch (e) {
    document.getElementById('recover-v1-status').textContent = '恢复失败: ' + e;
    document.getElementById('recover-v1-status').classList.remove('hidden');
    document.getElementById('recover-v1-status').classList.add('text-danger');
  }

  btn.textContent = '选择文件夹并恢复';
  btn.disabled = false;
}

// ── Manual Decrypt (orphaned folder not in list) ──
async function handleDecryptOrphaned() {
  try {
    const dialog = getDialog();
    if (!dialog) {
      toast('文件选择器不可用，请使用「加密新文件夹」页面的手动路径输入', 'error');
      return;
    }
    const path = await dialog.open({
      directory: true,
      multiple: false,
      title: '选择要解密的加密文件夹（包含隐藏的 .pw4lock 文件）',
    });
    if (!path) return;

    state.orphanFolderPath = path;
    document.getElementById('orphan-folder-path').textContent = '📁 ' + path;
    document.getElementById('orphan-password').value = '';
    document.getElementById('orphan-error').classList.add('hidden');
    document.getElementById('modal-orphan').classList.remove('hidden');
    document.getElementById('orphan-password').focus();
  } catch (e) {
    toast('选择文件夹失败: ' + e, 'error');
  }
}

function closeOrphanModal() {
  document.getElementById('modal-orphan').classList.add('hidden');
  state.orphanFolderPath = null;
}

async function handleDecryptOrphanedConfirm() {
  const path = state.orphanFolderPath;
  const password = document.getElementById('orphan-password').value;
  const errDiv = document.getElementById('orphan-error');
  const btn = document.getElementById('btn-orphan-decrypt');

  if (!path) {
    errDiv.textContent = '请先选择要解密的文件夹';
    errDiv.classList.remove('hidden');
    return;
  }
  if (!password) {
    errDiv.textContent = '请输入加密密码';
    errDiv.classList.remove('hidden');
    return;
  }

  btn.textContent = '⏳ ...';
  btn.disabled = true;
  errDiv.classList.add('hidden');

  try {
    const result = await invoke('decrypt_orphaned_folder', {
      args: { folderPath: path, password }
    });

    if (result.ok && result.data.status === 'success') {
      closeOrphanModal();
      await loadFolders();
      toast(`✅ 解密成功 — ${result.data.filesDecrypted} 个文件已恢复`, 'success');
    } else if (result.ok) {
      errDiv.textContent = result.data.message;
      errDiv.classList.remove('hidden');
    } else {
      errDiv.textContent = result.error || '解密失败';
      errDiv.classList.remove('hidden');
    }
  } catch (e) {
    errDiv.textContent = '调用失败: ' + e;
    errDiv.classList.remove('hidden');
  }

  btn.textContent = '🔓 解密';
  btn.disabled = false;
}

// ── Utilities ──────────────────────────────
function esc(str) {
  if (!str) return '';
  const div = document.createElement('div');
  div.textContent = str;
  return div.innerHTML;
}

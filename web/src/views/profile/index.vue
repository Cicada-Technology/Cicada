<script setup lang="ts">
/**
 * 个人中心：资料、改密、TOTP 与登录身份。
 *
 * 全部是「当前账号自助」接口，不需要任何系统权限点；改密成功后后端会撤销
 * 所有会话，因此这里主动结束本地会话并回到登录页。
 */
import { computed, onMounted, reactive, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { useRoute } from 'vue-router';

import {
  bindIdentity,
  changePassword,
  disableTotp,
  enableTotp,
  getTotpStatus,
  listIdentities,
  setupTotp,
  unbindIdentity,
  updateProfile,
  type IdentityVo,
  type TotpSetupResult,
} from '@/api/auth';
import AppDialog from '@/components/AppDialog.vue';
import AppPage from '@/components/AppPage.vue';
import { useConfirm } from '@/composables/useConfirm';
import { useAuthStore } from '@/stores/auth';
import { copyText } from '@/utils/download';
import { notifySuccess, notifyWarning } from '@/utils/feedback';

const { t } = useI18n();
const route = useRoute();
const auth = useAuthStore();
const { confirm } = useConfirm();

const title = computed(() => route.meta.title ?? '个人中心');

const profileForm = reactive({ nick_name: '', email: '', phonenumber: '' });
const savingProfile = ref(false);

function syncProfile(): void {
  profileForm.nick_name = auth.profile?.nick_name ?? '';
  profileForm.email = auth.profile?.email ?? '';
  profileForm.phonenumber = auth.profile?.phonenumber ?? '';
}

async function saveProfile(): Promise<void> {
  if (profileForm.nick_name.trim() === '') {
    notifyWarning('昵称不能为空');
    return;
  }
  savingProfile.value = true;
  try {
    await updateProfile({
      nick_name: profileForm.nick_name.trim(),
      email: profileForm.email.trim() || null,
      phonenumber: profileForm.phonenumber.trim() || null,
    });
    await auth.loadProfile();
    syncProfile();
    notifySuccess('资料已更新');
  } catch {
    // 提示由拦截器给出。
  } finally {
    savingProfile.value = false;
  }
}

const passwordForm = reactive({ old_password: '', new_password: '', confirm: '' });
const savingPassword = ref(false);

async function submitPassword(): Promise<void> {
  if (passwordForm.new_password.length < 8) {
    notifyWarning('新密码至少 8 位');
    return;
  }
  if (passwordForm.new_password !== passwordForm.confirm) {
    notifyWarning('两次输入的新密码不一致');
    return;
  }
  savingPassword.value = true;
  try {
    await changePassword({
      old_password: passwordForm.old_password,
      new_password: passwordForm.new_password,
    });
    notifySuccess('密码已更新，请重新登录');
    auth.reset('expired');
  } catch {
    // 提示由拦截器给出。
  } finally {
    savingPassword.value = false;
  }
}

const identities = ref<IdentityVo[]>([]);
const identityLoading = ref(false);
const bindForm = reactive({
  visible: false,
  identity_type: 'Phone',
  identifier: '',
  code: '',
  submitting: false,
});

async function loadIdentities(): Promise<void> {
  identityLoading.value = true;
  try {
    identities.value = await listIdentities();
  } finally {
    identityLoading.value = false;
  }
}

function openBind(): void {
  bindForm.identity_type = 'Phone';
  bindForm.identifier = '';
  bindForm.code = '';
  bindForm.visible = true;
}

async function submitBind(): Promise<void> {
  if (bindForm.identifier.trim() === '') {
    notifyWarning('身份标识不能为空');
    return;
  }
  bindForm.submitting = true;
  try {
    await bindIdentity({
      identity_type: bindForm.identity_type,
      identifier: bindForm.identifier.trim(),
      code: bindForm.code.trim() || null,
    });
    bindForm.visible = false;
    notifySuccess('身份已绑定');
    await loadIdentities();
  } catch {
    // 提示由拦截器给出。
  } finally {
    bindForm.submitting = false;
  }
}

async function removeIdentity(row: IdentityVo): Promise<void> {
  const agreed = await confirm(`确定解绑「${row.identifier}」吗？`, { danger: true });
  if (!agreed) return;
  try {
    await unbindIdentity(row.identity_id);
    notifySuccess('已解绑');
    await loadIdentities();
  } catch {
    // 提示由拦截器给出。
  }
}

const totpEnabled = ref(false);
const totpLoading = ref(false);
const totpSetup = ref<TotpSetupResult | null>(null);
const totpCode = ref('');
const totpDialog = reactive({ visible: false, mode: 'setup' as 'setup' | 'disable' });

async function loadTotp(): Promise<void> {
  totpLoading.value = true;
  try {
    totpEnabled.value = (await getTotpStatus()).enabled;
  } finally {
    totpLoading.value = false;
  }
}

async function openTotpSetup(): Promise<void> {
  try {
    totpSetup.value = await setupTotp();
    totpCode.value = '';
    totpDialog.mode = 'setup';
    totpDialog.visible = true;
  } catch {
    // 提示由拦截器给出。
  }
}

function openTotpDisable(): void {
  totpCode.value = '';
  totpDialog.mode = 'disable';
  totpDialog.visible = true;
}

async function submitTotp(): Promise<void> {
  if (totpCode.value.trim() === '') {
    notifyWarning('请输入 6 位动态码');
    return;
  }
  try {
    if (totpDialog.mode === 'setup') await enableTotp(totpCode.value.trim());
    else await disableTotp(totpCode.value.trim());
    totpDialog.visible = false;
    totpSetup.value = null;
    await loadTotp();
    notifySuccess(totpDialog.mode === 'setup' ? '二次验证已启用' : '二次验证已关闭');
  } catch {
    // 提示由拦截器给出。
  }
}

async function copySecret(): Promise<void> {
  if (!totpSetup.value) return;
  const ok = await copyText(totpSetup.value.secret);
  if (ok) notifySuccess('密钥已复制');
  else notifyWarning('复制失败，请手动抄写');
}

onMounted(async () => {
  await auth.loadProfile();
  syncProfile();
  await Promise.all([loadIdentities(), loadTotp()]);
});
</script>

<template>
  <AppPage :title="title" :description="'当前账号的资料、安全设置与登录身份。'">
    <div class="profile__grid">
      <section class="profile__card ci-card">
        <h2>资料</h2>
        <el-form label-width="90px" @submit.prevent>
          <el-form-item label="账号">
            <el-input :model-value="auth.profile?.user_name" disabled />
          </el-form-item>
          <el-form-item label="昵称" required>
            <el-input v-model="profileForm.nick_name" maxlength="64" />
          </el-form-item>
          <el-form-item label="邮箱">
            <el-input v-model="profileForm.email" clearable placeholder="用于接收通知" />
          </el-form-item>
          <el-form-item label="手机号">
            <el-input v-model="profileForm.phonenumber" clearable />
          </el-form-item>
          <el-form-item label="租户">
            <el-input :model-value="auth.profile?.tenant_id" disabled />
          </el-form-item>
        </el-form>
        <div class="profile__actions">
          <el-button type="primary" :loading="savingProfile" @click="saveProfile">
            {{ t('common.save') }}
          </el-button>
        </div>
      </section>

      <section class="profile__card ci-card">
        <h2>修改密码</h2>
        <el-form label-width="90px" @submit.prevent>
          <el-form-item label="当前密码" required>
            <el-input v-model="passwordForm.old_password" type="password" show-password />
          </el-form-item>
          <el-form-item label="新密码" required>
            <el-input v-model="passwordForm.new_password" type="password" show-password />
          </el-form-item>
          <el-form-item label="确认新密码" required>
            <el-input v-model="passwordForm.confirm" type="password" show-password />
          </el-form-item>
        </el-form>
        <div class="profile__actions">
          <el-button type="primary" :loading="savingPassword" @click="submitPassword">
            更新密码
          </el-button>
        </div>
      </section>
    </div>

    <section class="profile__card ci-card profile__wide">
      <div class="profile__section-head">
        <h2>登录身份</h2>
        <el-button type="primary" plain @click="openBind">绑定身份</el-button>
      </div>
      <el-table v-loading="identityLoading" :data="identities" row-key="identity_id">
        <el-table-column prop="identity_type" label="类型" width="130" />
        <el-table-column prop="identifier" label="标识" min-width="220" />
        <el-table-column label="主身份" width="90">
          <template #default="{ row }">{{ row.is_primary ? '是' : '否' }}</template>
        </el-table-column>
        <el-table-column label="已验证" width="90">
          <template #default="{ row }">
            <el-tag :type="row.verified ? 'success' : 'info'" size="small" effect="light" round>
              {{ row.verified ? '是' : '否' }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column :label="t('common.actions')" width="100">
          <template #default="{ row }">
            <el-button
              link
              type="danger"
              :disabled="row.is_primary"
              @click="removeIdentity(row)"
            >
              解绑
            </el-button>
          </template>
        </el-table-column>
      </el-table>
    </section>

    <section class="profile__card ci-card profile__wide">
      <div class="profile__section-head">
        <h2>二次验证（TOTP）</h2>
        <el-tag :type="totpEnabled ? 'success' : 'info'" effect="light" round>
          {{ totpEnabled ? '已启用' : '未启用' }}
        </el-tag>
      </div>
      <p class="profile__hint">
        启用后登录需要 6 位动态码；关闭或启用都要先验证一次当前动态码。
      </p>
      <div class="profile__actions">
        <el-button v-if="!totpEnabled" type="primary" :loading="totpLoading" @click="openTotpSetup">
          启用二次验证
        </el-button>
        <el-button v-else type="danger" plain :loading="totpLoading" @click="openTotpDisable">
          关闭二次验证
        </el-button>
      </div>
    </section>

    <AppDialog
      v-model="bindForm.visible"
      title="绑定登录身份"
      :confirm-loading="bindForm.submitting"
      @confirm="submitBind"
    >
      <el-form label-width="90px" @submit.prevent>
        <el-form-item label="类型">
          <el-select v-model="bindForm.identity_type" style="width: 100%">
            <el-option label="手机号" value="Phone" />
            <el-option label="邮箱" value="Email" />
          </el-select>
        </el-form-item>
        <el-form-item label="标识" required>
          <el-input v-model="bindForm.identifier" placeholder="手机号或邮箱" />
        </el-form-item>
        <el-form-item label="验证码">
          <el-input v-model="bindForm.code" placeholder="服务端配置了验证码通道时必填" />
        </el-form-item>
      </el-form>
    </AppDialog>

    <AppDialog
      v-model="totpDialog.visible"
      :title="totpDialog.mode === 'setup' ? '启用二次验证' : '关闭二次验证'"
      @confirm="submitTotp"
    >
      <template v-if="totpDialog.mode === 'setup' && totpSetup">
        <p class="profile__hint">用验证器 App 添加以下密钥，然后输入 6 位动态码。</p>
        <el-form label-width="90px" @submit.prevent>
          <el-form-item label="密钥">
            <div class="profile__secret">
              <code>{{ totpSetup.secret }}</code>
              <el-button link type="primary" @click="copySecret">复制</el-button>
            </div>
          </el-form-item>
          <el-form-item label="otpauth">
            <code class="profile__otpauth">{{ totpSetup.otpauth_url }}</code>
          </el-form-item>
          <el-form-item label="动态码" required>
            <el-input v-model="totpCode" maxlength="6" />
          </el-form-item>
        </el-form>
      </template>
      <el-form v-else label-width="90px" @submit.prevent>
        <el-form-item label="动态码" required>
          <el-input v-model="totpCode" maxlength="6" />
        </el-form-item>
      </el-form>
    </AppDialog>
  </AppPage>
</template>

<style scoped>
.profile__grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--ci-space-4);
}

.profile__card {
  padding: var(--ci-space-5);
  min-width: 0;
}

.profile__card h2 {
  margin-bottom: var(--ci-space-4);
  font-size: var(--ci-text-lg);
  font-weight: var(--ci-weight-semibold);
  color: var(--ci-color-text);
}

.profile__wide {
  margin-top: var(--ci-space-4);
}

.profile__section-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--ci-space-3);
  margin-bottom: var(--ci-space-3);
}

.profile__section-head h2 {
  margin: 0;
}

.profile__actions {
  display: flex;
  justify-content: flex-end;
  margin-top: var(--ci-space-3);
}

.profile__hint {
  font-size: var(--ci-text-sm);
  color: var(--ci-color-text-secondary);
  line-height: var(--ci-leading-relaxed);
}

.profile__secret {
  display: flex;
  align-items: center;
  gap: var(--ci-space-2);
  min-width: 0;
}

.profile__secret code,
.profile__otpauth {
  font-family: var(--ci-font-mono);
  font-size: var(--ci-text-xs);
  color: var(--ci-color-text-secondary);
  overflow-wrap: anywhere;
}

@media (max-width: 900px) {
  .profile__grid {
    grid-template-columns: 1fr;
  }
}
</style>

import { useSettingsStore } from '../stores/settingsStore';
import { useUsageStore } from '../stores/usageStore';

export const CRASH_RECOVERY_KEY = 'incubar-crash-recovery';

const removeCrashRecoveryFlag = () => {
  try {
    localStorage.removeItem(CRASH_RECOVERY_KEY);
  } catch (error) {
    console.warn('Failed to clear crash recovery flag:', error);
  }
};

const safeResetUsageStore = () => {
  const usageStore = useUsageStore.getState();
  try {
    usageStore.resetState();
  } catch (error) {
    console.warn('Failed to reset usage state:', error);
  }
};

const safeResetSettingsStore = () => {
  const settingsStore = useSettingsStore.getState();
  try {
    settingsStore.resetToDefaults();
  } catch (error) {
    console.warn('Failed to reset settings state:', error);
  }
};

export const restoreSafeStateAfterCrash = () => {
  try {
    const recoveryFlag = localStorage.getItem(CRASH_RECOVERY_KEY);
    if (!recoveryFlag) {
      return;
    }

    safeResetUsageStore();
    safeResetSettingsStore();
    useSettingsStore.getState().setCrashRecoveryAt(recoveryFlag);
  } catch (error) {
    console.warn('Failed to restore safe state after crash:', error);
  } finally {
    removeCrashRecoveryFlag();
  }
};

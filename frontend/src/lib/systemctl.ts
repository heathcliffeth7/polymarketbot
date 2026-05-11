import { exec } from 'child_process';
import { promisify } from 'util';

const execAsync = promisify(exec);
const DEFAULT_SERVICE_NAME = 'dextrabot';
const rawServiceName = process.env.BOT_SERVICE_NAME || DEFAULT_SERVICE_NAME;
const serviceName = /^[a-zA-Z0-9_.@-]+$/.test(rawServiceName)
  ? rawServiceName
  : DEFAULT_SERVICE_NAME;

const CONTROL_DISABLED_MESSAGE =
  'Service control is disabled by SYSTEMD_CONTROL_ENABLED=false.';
const SUDO_UNAVAILABLE_MESSAGE =
  'Passwordless sudo is required for service control (sudo -n). Configure sudoers or restart manually.';

export type ServiceAction = 'start' | 'stop' | 'restart';
export type ControlReasonCode =
  | 'disabled_by_env'
  | 'systemctl_missing'
  | 'sudo_unavailable'
  | 'platform_unsupported';

export interface ControlCapability {
  available: boolean;
  reasonCode: ControlReasonCode | null;
  reason: string | null;
}

export interface ServiceStatus {
  serviceActive: boolean;
  controlAvailable: boolean;
  controlReason: string | null;
  controlReasonCode: ControlReasonCode | null;
}

export interface ControlResult {
  success: boolean;
  message: string;
  controlAvailable: boolean;
  controlReason: string | null;
  controlReasonCode: ControlReasonCode | null;
}

function parseControlEnabled(): boolean {
  const raw = (process.env.SYSTEMD_CONTROL_ENABLED || 'true').trim().toLowerCase();
  return !['0', 'false', 'no', 'off'].includes(raw);
}

async function commandExists(command: string): Promise<boolean> {
  try {
    await execAsync(`command -v ${command}`);
    return true;
  } catch {
    return false;
  }
}

interface ExecCommandError extends Error {
  stdout?: string;
  stderr?: string;
}

function getCommandErrorFields(err: unknown): { message: string; stdout: string; stderr: string } {
  if (err instanceof Error) {
    const typed = err as ExecCommandError;
    return {
      message: typed.message,
      stdout: typed.stdout || '',
      stderr: typed.stderr || '',
    };
  }

  return {
    message: 'Unknown command error',
    stdout: '',
    stderr: '',
  };
}

function isSudoUnavailable(message: string, stderr: string): boolean {
  const combined = `${message}\n${stderr}`.toLowerCase();
  return (
    combined.includes('a password is required') ||
    combined.includes('sudo:') && combined.includes('password') ||
    combined.includes('not in the sudoers') ||
    combined.includes('permission denied')
  );
}

export async function checkControlCapability(): Promise<ControlCapability> {
  if (!parseControlEnabled()) {
    return {
      available: false,
      reasonCode: 'disabled_by_env',
      reason: CONTROL_DISABLED_MESSAGE,
    };
  }

  if (process.platform !== 'linux') {
    return {
      available: false,
      reasonCode: 'platform_unsupported',
      reason: 'Service control requires Linux + systemd.',
    };
  }

  const hasSystemctl = await commandExists('systemctl');
  if (!hasSystemctl) {
    return {
      available: false,
      reasonCode: 'systemctl_missing',
      reason: 'systemctl command is not available.',
    };
  }

  return {
    available: true,
    reasonCode: null,
    reason: null,
  };
}

export async function getServiceStatus(): Promise<ServiceStatus> {
  const capability = await checkControlCapability();
  if (!capability.available) {
    return {
      serviceActive: false,
      controlAvailable: false,
      controlReason: capability.reason,
      controlReasonCode: capability.reasonCode,
    };
  }

  try {
    const { stdout } = await execAsync(`sudo -n systemctl is-active ${serviceName}`);
    return {
      serviceActive: stdout.trim() === 'active',
      controlAvailable: true,
      controlReason: null,
      controlReasonCode: null,
    };
  } catch (err) {
    const { message, stdout, stderr } = getCommandErrorFields(err);
    if (isSudoUnavailable(message, stderr)) {
      return {
        serviceActive: false,
        controlAvailable: false,
        controlReason: SUDO_UNAVAILABLE_MESSAGE,
        controlReasonCode: 'sudo_unavailable',
      };
    }

    const status = stdout.trim();
    if (status) {
      return {
        serviceActive: status === 'active',
        controlAvailable: true,
        controlReason: null,
        controlReasonCode: null,
      };
    }

    return {
      serviceActive: false,
      controlAvailable: true,
      controlReason: message || 'Failed to read service status.',
      controlReasonCode: null,
    };
  }
}

export async function controlService(action: ServiceAction): Promise<ControlResult> {
  const allowed: ServiceAction[] = ['start', 'stop', 'restart'];
  if (!allowed.includes(action)) {
    return {
      success: false,
      message: `Invalid action: ${action}`,
      controlAvailable: true,
      controlReason: null,
      controlReasonCode: null,
    };
  }

  const capability = await checkControlCapability();
  if (!capability.available) {
    return {
      success: false,
      message: capability.reason || 'Service control is unavailable.',
      controlAvailable: false,
      controlReason: capability.reason,
      controlReasonCode: capability.reasonCode,
    };
  }

  if (action === 'start') {
    const status = await getServiceStatus();
    if (status.controlAvailable && status.serviceActive) {
      return {
        success: true,
        message: 'Runner zaten aktif; yeni proses acilmadi, mevcut singleton proses kullanilacak.',
        controlAvailable: true,
        controlReason: null,
        controlReasonCode: null,
      };
    }
  }

  try {
    await execAsync(`sudo -n systemctl ${action} ${serviceName}`);
    return {
      success: true,
      message: `Service ${action} successful`,
      controlAvailable: true,
      controlReason: null,
      controlReasonCode: null,
    };
  } catch (err) {
    const { message, stderr } = getCommandErrorFields(err);
    if (isSudoUnavailable(message, stderr)) {
      return {
        success: false,
        message: SUDO_UNAVAILABLE_MESSAGE,
        controlAvailable: false,
        controlReason: SUDO_UNAVAILABLE_MESSAGE,
        controlReasonCode: 'sudo_unavailable',
      };
    }

    return {
      success: false,
      message,
      controlAvailable: true,
      controlReason: message,
      controlReasonCode: null,
    };
  }
}

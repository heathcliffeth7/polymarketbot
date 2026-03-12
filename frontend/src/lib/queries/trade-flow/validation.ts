import {
  isValidTelegramChatTarget,
  readEffectiveClaimConfigForServer,
  readTelegramBotTokenForServer,
  readTelegramChatIdForServer,
  type UserConfigContext,
} from '@/lib/config';
import type { TradeFlowValidationResult, TradeFlowValidationIssue } from '@/lib/types';
import { SUPPORTED_NODE_TYPES, isRecord, toBooleanish } from './shared';
import {
  collectReachableFromTriggers,
  collectRootNodeKeys,
  detectCycles,
  normalizeTradeFlowGraph,
} from './graph';
import { validateNodeConfig } from './validation-actions';

export function validateTradeFlowGraph(graphJson: unknown): TradeFlowValidationResult {
  const graph = normalizeTradeFlowGraph(graphJson);
  const issues: TradeFlowValidationIssue[] = [];

  if (!isRecord(graph.context)) {
    issues.push({
      severity: 'error',
      code: 'invalid_context',
      message: 'Graph context must be an object.',
    });
  } else if (
    graph.context.autoClaimEnabled != null &&
    toBooleanish(graph.context.autoClaimEnabled) == null
  ) {
    issues.push({
      severity: 'error',
      code: 'invalid_auto_claim_enabled',
      message: 'Graph context autoClaimEnabled must be boolean (true/false).',
    });
  }

  const nodeKeySet = new Set<string>();
  for (const node of graph.nodes) {
    if (nodeKeySet.has(node.key)) {
      issues.push({
        severity: 'error',
        code: 'duplicate_node_key',
        message: `Node key already exists: ${node.key}`,
        nodeKey: node.key,
      });
      continue;
    }
    nodeKeySet.add(node.key);

    if (!SUPPORTED_NODE_TYPES.has(node.type)) {
      issues.push({
        severity: 'warning',
        code: 'unknown_node_type',
        message: `Unsupported/unknown node type: ${node.type}`,
        nodeKey: node.key,
      });
    }

    validateNodeConfig(issues, node, graph);
  }

  const edgeKeySet = new Set<string>();
  for (const edge of graph.edges) {
    if (edgeKeySet.has(edge.key)) {
      issues.push({
        severity: 'error',
        code: 'duplicate_edge_key',
        message: `Edge key already exists: ${edge.key}`,
        edgeKey: edge.key,
      });
    }
    edgeKeySet.add(edge.key);

    if (!nodeKeySet.has(edge.source)) {
      issues.push({
        severity: 'error',
        code: 'edge_source_missing',
        message: `Edge source node not found: ${edge.source}`,
        edgeKey: edge.key,
      });
    }
    if (!nodeKeySet.has(edge.target)) {
      issues.push({
        severity: 'error',
        code: 'edge_target_missing',
        message: `Edge target node not found: ${edge.target}`,
        edgeKey: edge.key,
      });
    }
  }

  const triggerCount = graph.nodes.filter((node) => node.type.startsWith('trigger.')).length;
  const actionCount = graph.nodes.filter((node) => node.type.startsWith('action.')).length;
  const rootNodeKeys = collectRootNodeKeys(graph.nodes, graph.edges);

  if (triggerCount === 0) {
    const hasDualDcaNode = graph.nodes.some((node) => node.type === 'action.dual_dca');
    if (!hasDualDcaNode) {
      issues.push({
        severity: 'error',
        code: 'missing_trigger',
        message: 'At least one trigger node is required.',
      });
    } else {
      const invalidRootNodes = graph.nodes
        .filter((node) => rootNodeKeys.has(node.key) && node.type !== 'action.dual_dca')
        .map((node) => node.key);
      if (invalidRootNodes.length > 0) {
        issues.push({
          severity: 'error',
          code: 'missing_trigger_invalid_roots_for_dual_dca',
          message: `Trigger yoksa root node'lar sadece action.dual_dca olabilir: ${invalidRootNodes.join(', ')}`,
        });
      }
    }
  }
  if (actionCount === 0) {
    issues.push({
      severity: 'error',
      code: 'missing_action',
      message: 'At least one action node is required.',
    });
  }

  for (const node of graph.nodes) {
    if (node.type !== 'logic.if') continue;
    const outgoing = graph.edges.filter((edge) => edge.source === node.key);
    const hasTrue = outgoing.some((edge) => edge.type === 'on_true');
    const hasFalse = outgoing.some((edge) => edge.type === 'on_false');
    if (!hasTrue || !hasFalse) {
      issues.push({
        severity: 'warning',
        code: 'if_missing_branch',
        message: `If node should include both on_true and on_false branches: ${node.key}`,
        nodeKey: node.key,
      });
    }
  }

  for (const node of graph.nodes) {
    if (node.type !== 'logic.switch') continue;
    const outgoing = graph.edges.filter((edge) => edge.source === node.key);
    const hasDefault = outgoing.some((edge) => edge.type === 'default');
    if (!hasDefault) {
      issues.push({
        severity: 'warning',
        code: 'switch_missing_default',
        message: `Switch node should include default branch: ${node.key}`,
        nodeKey: node.key,
      });
    }
  }

  for (const node of graph.nodes) {
    if (node.type !== 'trigger.market_price') continue;
    const config = isRecord(node.config) ? node.config : {};
    const hasOutcomeConditions =
      Array.isArray(config.outcomeConditions) && config.outcomeConditions.length > 0;
    if (!hasOutcomeConditions) continue;

    const defaultOutgoing = graph.edges.filter(
      (edge) => edge.source === node.key && edge.type === 'default'
    );
    if (defaultOutgoing.length <= 1) continue;

    for (const edge of defaultOutgoing) {
      if (edge.condition) continue;
      issues.push({
        severity: 'warning',
        code: 'market_price_branch_condition_missing',
        message:
          `Multi-outcome trigger.market_price branch should define an edge condition to avoid firing multiple workflow branches: ${edge.key}`,
        nodeKey: node.key,
        edgeKey: edge.key,
      });
    }
  }

  if (graph.nodes.length > 0 && detectCycles(graph.nodes, graph.edges)) {
    issues.push({
      severity: 'error',
      code: 'cycle_detected',
      message: 'Graph contains cycle(s). In this version, cyclic flow is not allowed.',
    });
  }

  const reachable = collectReachableFromTriggers(graph.nodes, graph.edges);
  for (const node of graph.nodes) {
    if (!reachable.has(node.key)) {
      issues.push({
        severity: 'warning',
        code: 'unreachable_node',
        message: `Node is unreachable from start node(s): ${node.key}`,
        nodeKey: node.key,
      });
    }
  }

  const valid = !issues.some((issue) => issue.severity === 'error');
  return {
    valid,
    issues,
    stats: {
      nodes: graph.nodes.length,
      edges: graph.edges.length,
      triggers: triggerCount,
      actions: actionCount,
    },
  };
}

export async function validateTradeFlowGraphWithRuntimeConfig(
  graphJson: unknown,
  context: UserConfigContext
): Promise<TradeFlowValidationResult> {
  const graph = normalizeTradeFlowGraph(graphJson);
  const baseValidation = validateTradeFlowGraph(graph);
  const issues = [...baseValidation.issues];
  const autoClaimEnabled =
    isRecord(graph.context) && toBooleanish(graph.context.autoClaimEnabled) === true;
  const telegramNodes = graph.nodes.filter((node) => node.type === 'action.telegram_notify');

  if (telegramNodes.length > 0) {
    let userTelegramBotToken = '';
    let userTelegramDefaultChatId = '';
    let userTelegramReadError: string | null = null;
    try {
      userTelegramBotToken = (await readTelegramBotTokenForServer(context)).trim();
      userTelegramDefaultChatId = (await readTelegramChatIdForServer(context)).trim();
    } catch (err) {
      userTelegramReadError =
        err instanceof Error ? err.message : 'Failed to read telegram config';
    }

    for (const node of telegramNodes) {
      const config = isRecord(node.config) ? node.config : {};
      const nodeChatId = String(config.chatId ?? '').trim();

      if (userTelegramReadError) {
        issues.push({
          severity: 'error',
          code: 'telegram_config_invalid',
          message: `Telegram config okunamadi: ${userTelegramReadError}`,
          nodeKey: node.key,
        });
        continue;
      }

      if (!userTelegramBotToken) {
        issues.push({
          severity: 'error',
          code: 'missing_telegram_bot_token',
          message: 'action.telegram_notify requires a Telegram bot token in Settings -> Telegram for the current user.',
          nodeKey: node.key,
        });
      }

      if (!nodeChatId && !userTelegramDefaultChatId) {
        issues.push({
          severity: 'error',
          code: 'missing_telegram_chat_id',
          message:
            'action.telegram_notify requires chatId in node config or a default Telegram chat_id in Settings -> Telegram for the current user.',
          nodeKey: node.key,
        });
      }

      if (nodeChatId && !isValidTelegramChatTarget(nodeChatId)) {
        issues.push({
          severity: 'error',
          code: 'invalid_telegram_chat_id',
          message:
            'action.telegram_notify chatId must be a Telegram chat ID like -1001234567890 or a @channelusername.',
          nodeKey: node.key,
        });
      }

      if (!nodeChatId && userTelegramDefaultChatId && !isValidTelegramChatTarget(userTelegramDefaultChatId)) {
        issues.push({
          severity: 'error',
          code: 'invalid_default_telegram_chat_id',
          message:
            'Settings -> Telegram chat_id must be a Telegram chat ID like -1001234567890 or a @channelusername.',
          nodeKey: node.key,
        });
      }

      if (String(config.botToken ?? '').trim()) {
        issues.push({
          severity: 'warning',
          code: 'legacy_inline_telegram_bot_token',
          message:
            'Bu node eski inline botToken tasiyor, fakat artik kullanilmaz. Settings -> Telegram ekraninda kullanici tokenini tanimlayip node’u kaydederek yeni modele gec.',
          nodeKey: node.key,
        });
      }
    }
  }

  if (autoClaimEnabled) {
    try {
      const claim = await readEffectiveClaimConfigForServer(context);
      if (!claim.enabled) {
        issues.push({
          severity: 'error',
          code: 'auto_claim_enabled_but_claim_disabled',
          message:
            'autoClaimEnabled=true but claim config is disabled or missing. Go to Settings -> Claim and enable auto-claiming first.',
        });
      } else {
        const missingSources: string[] = [];
        if (!claim.hasRpcSource) missingSources.push('rpc_url/rpc_url_env');
        if (!claim.hasUserAddressSource) missingSources.push('user_address/user_address_env');
        if (!claim.hasPrivateKeySource) {
          missingSources.push('private_key/private_key_env');
        }
        if (claim.executionMode === 'builder_relayer') {
          if (!claim.hasSafeAddressSource) {
            missingSources.push('exchange.gnosis_safe_address');
          }
          if (!claim.hasBuilderCredsSource) {
            missingSources.push(
              'exchange.builder_api_key/builder_api_secret/builder_api_passphrase'
            );
          }
        }
        if (missingSources.length > 0) {
          issues.push({
            severity: 'error',
            code: 'auto_claim_enabled_but_claim_incomplete',
            message: `autoClaimEnabled=true but claim config is incomplete: missing ${missingSources.join(', ')}.`,
          });
        }
      }
    } catch {
      issues.push({
        severity: 'error',
        code: 'auto_claim_config_read_failed',
        message: 'Failed to read claim config for validation.',
      });
    }
  }

  return {
    ...baseValidation,
    valid: !issues.some((issue) => issue.severity === 'error'),
    issues,
  };
}

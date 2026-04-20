#!/usr/bin/env node
import readline from 'node:readline';

const API_BASE = 'https://api.sgroup.qq.com';
const TOKEN_URL = process.env.FIN_QQBOT_TOKEN_URL || 'https://bots.qq.com/app/getAppAccessToken';
const INTENTS = {
  PUBLIC_GUILD_MESSAGES: 1 << 30,
  DIRECT_MESSAGE: 1 << 12,
  GROUP_AND_C2C: 1 << 25,
};
const INTENT_LEVELS = [
  {
    name: 'full',
    intents: INTENTS.PUBLIC_GUILD_MESSAGES | INTENTS.DIRECT_MESSAGE | INTENTS.GROUP_AND_C2C,
  },
  {
    name: 'group+channel',
    intents: INTENTS.PUBLIC_GUILD_MESSAGES | INTENTS.GROUP_AND_C2C,
  },
  {
    name: 'channel-only',
    intents: INTENTS.PUBLIC_GUILD_MESSAGES,
  },
];

const state = {
  appId: '',
  clientSecret: '',
  ws: null,
  heartbeatTimer: null,
  reconnectTimer: null,
  stopping: false,
  token: null,
  tokenExpiresAt: 0,
  sessionId: null,
  lastSeq: null,
  intentLevelIndex: 0,
  lastSuccessfulIntentLevel: -1,
};

let stdoutBroken = false;

function emit(obj) {
  if (stdoutBroken) {
    return;
  }
  try {
    process.stdout.write(`${JSON.stringify(obj)}\n`);
  } catch (err) {
    if (err?.code === 'EPIPE' || err?.code === 'ERR_STREAM_DESTROYED') {
      stdoutBroken = true;
      state.stopping = true;
      clearConnection();
      process.exit(0);
      return;
    }
    throw err;
  }
}

function log(message, extra) {
  const suffix = extra === undefined ? '' : ` ${JSON.stringify(extra)}`;
  process.stderr.write(`[qqbot-peer] ${message}${suffix}\n`);
}

process.stdout.on('error', (err) => {
  if (err?.code === 'EPIPE' || err?.code === 'ERR_STREAM_DESTROYED') {
    stdoutBroken = true;
    state.stopping = true;
    clearConnection();
    process.exit(0);
    return;
  }
  throw err;
});

function nextMsgSeq(msgId) {
  const seed = msgId ? Array.from(msgId).reduce((acc, ch) => acc + ch.charCodeAt(0), 0) : 0;
  return ((Date.now() % 65535) ^ seed) % 65535 || 1;
}

async function requestJson(url, init, label) {
  const response = await fetch(url, init);
  const text = await response.text();
  let body = {};
  if (text.trim()) {
    try {
      body = JSON.parse(text);
    } catch (err) {
      throw new Error(`${label} invalid json: ${String(err)}`);
    }
  }
  if (!response.ok) {
    throw new Error(`${label} failed (${response.status}): ${JSON.stringify(body).slice(0, 300)}`);
  }
  return body;
}

async function getAccessToken(forceRefresh = false) {
  if (!forceRefresh && state.token && Date.now() < state.tokenExpiresAt - 5 * 60 * 1000) {
    return state.token;
  }
  const body = await requestJson(
    TOKEN_URL,
    {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'User-Agent': 'fin-qqbot-peer/0.1',
      },
      body: JSON.stringify({ appId: state.appId, clientSecret: state.clientSecret }),
    },
    'token_request',
  );
  if (!body.access_token) {
    throw new Error(`token_request missing access_token: ${JSON.stringify(body).slice(0, 240)}`);
  }
  state.token = body.access_token;
  state.tokenExpiresAt = Date.now() + ((body.expires_in || 7200) * 1000);
  return state.token;
}

async function apiRequest(method, path, body, retry = true) {
  const token = await getAccessToken(false);
  try {
    return await requestJson(
      `${API_BASE}${path}`,
      {
        method,
        headers: {
          Authorization: `QQBot ${token}`,
          'Content-Type': 'application/json',
        },
        body: body ? JSON.stringify(body) : undefined,
      },
      `api:${path}`,
    );
  } catch (err) {
    if (!retry || !String(err).includes('(401)')) {
      throw err;
    }
    await getAccessToken(true);
    return apiRequest(method, path, body, false);
  }
}

async function getGatewayUrl() {
  const body = await apiRequest('GET', '/gateway');
  if (!body.url) {
    throw new Error(`gateway response missing url: ${JSON.stringify(body).slice(0, 240)}`);
  }
  return body.url;
}

async function sendText(payload) {
  const { to, text, replyToId } = payload || {};
  if (!to || !text) {
    throw new Error('send requires to + text');
  }
  const [kind, target] = String(to).split(':').slice(1);
  const body = {
    content: String(text),
    msg_type: 0,
    msg_seq: nextMsgSeq(replyToId),
    ...(replyToId ? { msg_id: replyToId } : {}),
  };
  if (kind === 'c2c') {
    return apiRequest('POST', `/v2/users/${target}/messages`, body);
  }
  if (kind === 'group') {
    return apiRequest('POST', `/v2/groups/${target}/messages`, body);
  }
  if (kind === 'channel') {
    return apiRequest('POST', `/channels/${target}/messages`, {
      content: String(text),
      ...(replyToId ? { msg_id: replyToId } : {}),
    });
  }
  throw new Error(`unsupported target: ${to}`);
}

function clearConnection() {
  if (state.heartbeatTimer) {
    clearInterval(state.heartbeatTimer);
    state.heartbeatTimer = null;
  }
  if (state.reconnectTimer) {
    clearTimeout(state.reconnectTimer);
    state.reconnectTimer = null;
  }
  if (state.ws) {
    try {
      state.ws.close();
    } catch {}
    state.ws = null;
  }
}

function scheduleReconnect(delayMs = 3000) {
  if (state.stopping || state.reconnectTimer) {
    return;
  }
  state.reconnectTimer = setTimeout(() => {
    state.reconnectTimer = null;
    connect().catch((err) => {
      emit({ event: 'error', data: { phase: 'connect', message: String(err) } });
      scheduleReconnect(5000);
    });
  }, delayMs);
}

function emitInbound(kind, payload) {
  emit({
    event: 'message',
    data: {
      type: kind,
      senderId: payload.senderId,
      ...(payload.senderName ? { senderName: payload.senderName } : {}),
      content: payload.content || '',
      messageId: payload.messageId,
      timestamp: payload.timestamp,
      ...(payload.groupOpenid ? { groupOpenid: payload.groupOpenid } : {}),
      ...(payload.channelId ? { channelId: payload.channelId } : {}),
      ...(payload.guildId ? { guildId: payload.guildId } : {}),
      ...(Array.isArray(payload.attachments) ? { attachments: payload.attachments } : {}),
    },
  });
}

function handleDispatch(eventType, data) {
  log('dispatch', {
    eventType,
    messageId: data?.id ?? null,
    timestamp: data?.timestamp ?? null,
  });
  if (eventType === 'READY') {
    state.sessionId = data.session_id || null;
    state.lastSuccessfulIntentLevel = state.intentLevelIndex;
    emit({
      event: 'ready',
      data: {
        sessionId: state.sessionId,
        intentLevel: INTENT_LEVELS[state.intentLevelIndex]?.name || 'unknown',
      },
    });
    return;
  }
  if (eventType === 'RESUMED') {
    emit({
      event: 'ready',
      data: {
        sessionId: state.sessionId,
        resumed: true,
        intentLevel: INTENT_LEVELS[state.lastSuccessfulIntentLevel >= 0 ? state.lastSuccessfulIntentLevel : state.intentLevelIndex]?.name || 'unknown',
      },
    });
    return;
  }
  if (eventType === 'C2C_MESSAGE_CREATE') {
    emitInbound('c2c', {
      senderId: data.author?.user_openid,
      content: data.content,
      messageId: data.id,
      timestamp: data.timestamp,
      attachments: data.attachments,
    });
    return;
  }
  if (eventType === 'GROUP_AT_MESSAGE_CREATE') {
    emitInbound('group', {
      senderId: data.author?.member_openid,
      content: data.content,
      messageId: data.id,
      timestamp: data.timestamp,
      groupOpenid: data.group_openid,
      attachments: data.attachments,
    });
    return;
  }
  if (eventType === 'AT_MESSAGE_CREATE') {
    emitInbound('guild', {
      senderId: data.author?.id,
      senderName: data.author?.username,
      content: data.content,
      messageId: data.id,
      timestamp: data.timestamp,
      channelId: data.channel_id,
      guildId: data.guild_id,
      attachments: data.attachments,
    });
    return;
  }
  if (eventType === 'DIRECT_MESSAGE_CREATE') {
    emitInbound('dm', {
      senderId: data.author?.id,
      senderName: data.author?.username,
      content: data.content,
      messageId: data.id,
      timestamp: data.timestamp,
      guildId: data.guild_id,
      attachments: data.attachments,
    });
    return;
  }
  log('dispatch-unhandled', { eventType });
}

function identifyPayload(accessToken) {
  const levelIndex = state.lastSuccessfulIntentLevel >= 0 ? state.lastSuccessfulIntentLevel : state.intentLevelIndex;
  const level = INTENT_LEVELS[Math.min(levelIndex, INTENT_LEVELS.length - 1)];
  return {
    op: 2,
    d: {
      token: `QQBot ${accessToken}`,
      intents: level.intents,
      shard: [0, 1],
    },
  };
}

async function connect() {
  clearConnection();
  const accessToken = await getAccessToken(false);
  const gatewayUrl = await getGatewayUrl();
  const ws = new WebSocket(gatewayUrl);
  state.ws = ws;

  ws.addEventListener('open', () => {
    log('websocket connected');
  });

  ws.addEventListener('message', async (event) => {
    try {
      const payload = JSON.parse(String(event.data));
      if (payload.s !== undefined && payload.s !== null) {
        state.lastSeq = payload.s;
      }
      if (payload.op === 10) {
        const hello = payload.d || {};
        if (state.sessionId && state.lastSeq !== null) {
          ws.send(JSON.stringify({
            op: 6,
            d: {
              token: `QQBot ${accessToken}`,
              session_id: state.sessionId,
              seq: state.lastSeq,
            },
          }));
        } else {
          ws.send(JSON.stringify(identifyPayload(accessToken)));
        }
        if (state.heartbeatTimer) {
          clearInterval(state.heartbeatTimer);
        }
        state.heartbeatTimer = setInterval(() => {
          if (ws.readyState === WebSocket.OPEN) {
            ws.send(JSON.stringify({ op: 1, d: state.lastSeq }));
          }
        }, hello.heartbeat_interval || 30000);
        return;
      }
      if (payload.op === 0) {
        handleDispatch(payload.t, payload.d || {});
        return;
      }
      if (payload.op === 7) {
        emit({ event: 'error', data: { phase: 'server_reconnect', message: 'server requested reconnect' } });
        clearConnection();
        scheduleReconnect(2000);
        return;
      }
      if (payload.op === 9) {
        const canResume = Boolean(payload.d);
        if (!canResume) {
          state.sessionId = null;
          state.lastSeq = null;
          if (state.intentLevelIndex < INTENT_LEVELS.length - 1) {
            state.intentLevelIndex += 1;
          }
        }
        emit({ event: 'error', data: { phase: 'invalid_session', canResume } });
        clearConnection();
        scheduleReconnect(3000);
      }
    } catch (err) {
      emit({ event: 'error', data: { phase: 'message_parse', message: String(err) } });
    }
  });

  ws.addEventListener('close', (event) => {
    if (state.stopping) {
      emit({ event: 'stopped', data: { code: event.code, reason: String(event.reason || '') } });
      return;
    }
    if ([4006, 4007, 4009].includes(event.code)) {
      state.sessionId = null;
      state.lastSeq = null;
    }
    if (event.code === 4004) {
      state.token = null;
      state.tokenExpiresAt = 0;
    }
    emit({ event: 'error', data: { phase: 'close', code: event.code, reason: String(event.reason || '') } });
    scheduleReconnect(event.code === 4008 ? 60000 : 3000);
  });

  ws.addEventListener('error', (event) => {
    emit({ event: 'error', data: { phase: 'websocket', message: String(event.message || 'websocket error') } });
  });
}

async function handleRequest(line) {
  let request;
  try {
    request = JSON.parse(line);
  } catch (err) {
    emit({ ok: false, error: `invalid json: ${String(err)}` });
    return;
  }
  const { action, payload, requestId } = request;
  try {
    if (action === 'start') {
      state.appId = String(payload?.appId || '').trim();
      state.clientSecret = String(payload?.clientSecret || '').trim();
      state.stopping = false;
      if (!state.appId || !state.clientSecret) {
        throw new Error('start requires appId + clientSecret');
      }
      await connect();
      emit({ ok: true, requestId, result: { state: 'starting' } });
      return;
    }
    if (action === 'send') {
      const result = await sendText(payload);
      emit({ ok: true, requestId, result: { id: result.id || null } });
      return;
    }
    if (action === 'stop') {
      state.stopping = true;
      clearConnection();
      emit({ ok: true, requestId, result: { state: 'stopped' } });
      emit({ event: 'stopped', data: { reason: 'requested' } });
      return;
    }
    emit({ ok: false, requestId, error: `unsupported action: ${action}` });
  } catch (err) {
    emit({ ok: false, requestId, error: String(err) });
  }
}

const rl = readline.createInterface({ input: process.stdin, crlfDelay: Infinity });
rl.on('line', (line) => {
  if (line.trim()) {
    handleRequest(line).catch((err) => emit({ ok: false, error: String(err) }));
  }
});
process.on('SIGTERM', () => {
  state.stopping = true;
  clearConnection();
  process.exit(0);
});
process.on('SIGINT', () => {
  state.stopping = true;
  clearConnection();
  process.exit(0);
});

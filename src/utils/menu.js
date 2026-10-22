import is from './is';

export const handleCommand = async (item) => {
  await handleCommandBefore(item);

  const args = item['command-arg']
    ? [item.command, item['command-arg']]
    : [item.command];

  await window.application.sendCommandToAll(...args);

  await handleCommandAfter(item);
};

async function handleCommandBefore(item) {
  console.log('handleCommandBefore', item);
  if (!item['command-before']) {
    return;
  }
  const [command, params] = item['command-before'].split('?');
  const args = parse(params);
  console.log('handleCommandBefore', command, args);
  await window.application.sendCommandToAll(command, args);
}

async function handleCommandAfter(item) {
  if (!item['command-after']) {
    return;
  }
  const [command, params] = item['command-after'].split('?');
  const args = parse(params);
  await window.application.sendCommandToAll(command, args);
}

const parse = (query) => {
  if (!query) {
    return {};
  }

  return Object.fromEntries(
    query
      .split('&')
      .filter(Boolean)
      .map((pair) => {
        const [key, value] = pair.split('=');
        return [key, value === undefined ? true : value];
      })
  );
};

export const acceleratorFor = (command, keystrokes) => {
  const keystroke = keystrokes[command];
  if (!keystroke) {
    return undefined;
  }

  const tokens = keystroke.split(/-(?=.)/);
  const key = tokens
    .pop()
    .toUpperCase()
    .replace('+', 'Plus')
    .replace('MINUS', '-');

  const modifiers = tokens.map((token) => {
    switch (token.toLowerCase()) {
      case 'cmdctrl':
        return is.macOS() ? 'Cmd' : 'Ctrl';
      case 'cmd':
        return 'Cmd';
      case 'ctrl':
        return 'Ctrl';
      case 'shift':
        return 'Shift';
      case 'alt':
        return 'Alt';
      default:
        return token;
    }
  });

  return [...modifiers, key].join('+');
};

export const translate = (node, i18n) => {
  const key = node.id || '';
  return key ? i18n.t(key) : '';
};

/******/ (() => { // webpackBootstrap
/******/ 	var __webpack_modules__ = ({

/***/ "./node_modules/electron-squirrel-startup/index.js"
/*!*********************************************************!*\
  !*** ./node_modules/electron-squirrel-startup/index.js ***!
  \*********************************************************/
(module, __unused_webpack_exports, __webpack_require__) {

var path = __webpack_require__(/*! path */ "path");
var spawn = (__webpack_require__(/*! child_process */ "child_process").spawn);
var debug = __webpack_require__(/*! debug */ "./node_modules/electron-squirrel-startup/node_modules/debug/src/index.js")('electron-squirrel-startup');
var app = (__webpack_require__(/*! electron */ "electron").app);

var run = function(args, done) {
  var updateExe = path.resolve(path.dirname(process.execPath), '..', 'Update.exe');
  debug('Spawning `%s` with args `%s`', updateExe, args);
  spawn(updateExe, args, {
    detached: true
  }).on('close', done);
};

var check = function() {
  if (process.platform === 'win32') {
    var cmd = process.argv[1];
    debug('processing squirrel command `%s`', cmd);
    var target = path.basename(process.execPath);

    if (cmd === '--squirrel-install' || cmd === '--squirrel-updated') {
      run(['--createShortcut=' + target + ''], app.quit);
      return true;
    }
    if (cmd === '--squirrel-uninstall') {
      run(['--removeShortcut=' + target + ''], app.quit);
      return true;
    }
    if (cmd === '--squirrel-obsolete') {
      app.quit();
      return true;
    }
  }
  return false;
};

module.exports = check();


/***/ },

/***/ "./node_modules/electron-squirrel-startup/node_modules/debug/src/browser.js"
/*!**********************************************************************************!*\
  !*** ./node_modules/electron-squirrel-startup/node_modules/debug/src/browser.js ***!
  \**********************************************************************************/
(module, exports, __webpack_require__) {

/**
 * This is the web browser implementation of `debug()`.
 *
 * Expose `debug()` as the module.
 */

exports = module.exports = __webpack_require__(/*! ./debug */ "./node_modules/electron-squirrel-startup/node_modules/debug/src/debug.js");
exports.log = log;
exports.formatArgs = formatArgs;
exports.save = save;
exports.load = load;
exports.useColors = useColors;
exports.storage = 'undefined' != typeof chrome
               && 'undefined' != typeof chrome.storage
                  ? chrome.storage.local
                  : localstorage();

/**
 * Colors.
 */

exports.colors = [
  'lightseagreen',
  'forestgreen',
  'goldenrod',
  'dodgerblue',
  'darkorchid',
  'crimson'
];

/**
 * Currently only WebKit-based Web Inspectors, Firefox >= v31,
 * and the Firebug extension (any Firefox version) are known
 * to support "%c" CSS customizations.
 *
 * TODO: add a `localStorage` variable to explicitly enable/disable colors
 */

function useColors() {
  // NB: In an Electron preload script, document will be defined but not fully
  // initialized. Since we know we're in Chrome, we'll just detect this case
  // explicitly
  if (typeof window !== 'undefined' && window.process && window.process.type === 'renderer') {
    return true;
  }

  // is webkit? http://stackoverflow.com/a/16459606/376773
  // document is undefined in react-native: https://github.com/facebook/react-native/pull/1632
  return (typeof document !== 'undefined' && document.documentElement && document.documentElement.style && document.documentElement.style.WebkitAppearance) ||
    // is firebug? http://stackoverflow.com/a/398120/376773
    (typeof window !== 'undefined' && window.console && (window.console.firebug || (window.console.exception && window.console.table))) ||
    // is firefox >= v31?
    // https://developer.mozilla.org/en-US/docs/Tools/Web_Console#Styling_messages
    (typeof navigator !== 'undefined' && navigator.userAgent && navigator.userAgent.toLowerCase().match(/firefox\/(\d+)/) && parseInt(RegExp.$1, 10) >= 31) ||
    // double check webkit in userAgent just in case we are in a worker
    (typeof navigator !== 'undefined' && navigator.userAgent && navigator.userAgent.toLowerCase().match(/applewebkit\/(\d+)/));
}

/**
 * Map %j to `JSON.stringify()`, since no Web Inspectors do that by default.
 */

exports.formatters.j = function(v) {
  try {
    return JSON.stringify(v);
  } catch (err) {
    return '[UnexpectedJSONParseError]: ' + err.message;
  }
};


/**
 * Colorize log arguments if enabled.
 *
 * @api public
 */

function formatArgs(args) {
  var useColors = this.useColors;

  args[0] = (useColors ? '%c' : '')
    + this.namespace
    + (useColors ? ' %c' : ' ')
    + args[0]
    + (useColors ? '%c ' : ' ')
    + '+' + exports.humanize(this.diff);

  if (!useColors) return;

  var c = 'color: ' + this.color;
  args.splice(1, 0, c, 'color: inherit')

  // the final "%c" is somewhat tricky, because there could be other
  // arguments passed either before or after the %c, so we need to
  // figure out the correct index to insert the CSS into
  var index = 0;
  var lastC = 0;
  args[0].replace(/%[a-zA-Z%]/g, function(match) {
    if ('%%' === match) return;
    index++;
    if ('%c' === match) {
      // we only are interested in the *last* %c
      // (the user may have provided their own)
      lastC = index;
    }
  });

  args.splice(lastC, 0, c);
}

/**
 * Invokes `console.log()` when available.
 * No-op when `console.log` is not a "function".
 *
 * @api public
 */

function log() {
  // this hackery is required for IE8/9, where
  // the `console.log` function doesn't have 'apply'
  return 'object' === typeof console
    && console.log
    && Function.prototype.apply.call(console.log, console, arguments);
}

/**
 * Save `namespaces`.
 *
 * @param {String} namespaces
 * @api private
 */

function save(namespaces) {
  try {
    if (null == namespaces) {
      exports.storage.removeItem('debug');
    } else {
      exports.storage.debug = namespaces;
    }
  } catch(e) {}
}

/**
 * Load `namespaces`.
 *
 * @return {String} returns the previously persisted debug modes
 * @api private
 */

function load() {
  var r;
  try {
    r = exports.storage.debug;
  } catch(e) {}

  // If debug isn't set in LS, and we're in Electron, try to load $DEBUG
  if (!r && typeof process !== 'undefined' && 'env' in process) {
    r = process.env.DEBUG;
  }

  return r;
}

/**
 * Enable namespaces listed in `localStorage.debug` initially.
 */

exports.enable(load());

/**
 * Localstorage attempts to return the localstorage.
 *
 * This is necessary because safari throws
 * when a user disables cookies/localstorage
 * and you attempt to access it.
 *
 * @return {LocalStorage}
 * @api private
 */

function localstorage() {
  try {
    return window.localStorage;
  } catch (e) {}
}


/***/ },

/***/ "./node_modules/electron-squirrel-startup/node_modules/debug/src/debug.js"
/*!********************************************************************************!*\
  !*** ./node_modules/electron-squirrel-startup/node_modules/debug/src/debug.js ***!
  \********************************************************************************/
(module, exports, __webpack_require__) {


/**
 * This is the common logic for both the Node.js and web browser
 * implementations of `debug()`.
 *
 * Expose `debug()` as the module.
 */

exports = module.exports = createDebug.debug = createDebug['default'] = createDebug;
exports.coerce = coerce;
exports.disable = disable;
exports.enable = enable;
exports.enabled = enabled;
exports.humanize = __webpack_require__(/*! ms */ "./node_modules/electron-squirrel-startup/node_modules/ms/index.js");

/**
 * The currently active debug mode names, and names to skip.
 */

exports.names = [];
exports.skips = [];

/**
 * Map of special "%n" handling functions, for the debug "format" argument.
 *
 * Valid key names are a single, lower or upper-case letter, i.e. "n" and "N".
 */

exports.formatters = {};

/**
 * Previous log timestamp.
 */

var prevTime;

/**
 * Select a color.
 * @param {String} namespace
 * @return {Number}
 * @api private
 */

function selectColor(namespace) {
  var hash = 0, i;

  for (i in namespace) {
    hash  = ((hash << 5) - hash) + namespace.charCodeAt(i);
    hash |= 0; // Convert to 32bit integer
  }

  return exports.colors[Math.abs(hash) % exports.colors.length];
}

/**
 * Create a debugger with the given `namespace`.
 *
 * @param {String} namespace
 * @return {Function}
 * @api public
 */

function createDebug(namespace) {

  function debug() {
    // disabled?
    if (!debug.enabled) return;

    var self = debug;

    // set `diff` timestamp
    var curr = +new Date();
    var ms = curr - (prevTime || curr);
    self.diff = ms;
    self.prev = prevTime;
    self.curr = curr;
    prevTime = curr;

    // turn the `arguments` into a proper Array
    var args = new Array(arguments.length);
    for (var i = 0; i < args.length; i++) {
      args[i] = arguments[i];
    }

    args[0] = exports.coerce(args[0]);

    if ('string' !== typeof args[0]) {
      // anything else let's inspect with %O
      args.unshift('%O');
    }

    // apply any `formatters` transformations
    var index = 0;
    args[0] = args[0].replace(/%([a-zA-Z%])/g, function(match, format) {
      // if we encounter an escaped % then don't increase the array index
      if (match === '%%') return match;
      index++;
      var formatter = exports.formatters[format];
      if ('function' === typeof formatter) {
        var val = args[index];
        match = formatter.call(self, val);

        // now we need to remove `args[index]` since it's inlined in the `format`
        args.splice(index, 1);
        index--;
      }
      return match;
    });

    // apply env-specific formatting (colors, etc.)
    exports.formatArgs.call(self, args);

    var logFn = debug.log || exports.log || console.log.bind(console);
    logFn.apply(self, args);
  }

  debug.namespace = namespace;
  debug.enabled = exports.enabled(namespace);
  debug.useColors = exports.useColors();
  debug.color = selectColor(namespace);

  // env-specific initialization logic for debug instances
  if ('function' === typeof exports.init) {
    exports.init(debug);
  }

  return debug;
}

/**
 * Enables a debug mode by namespaces. This can include modes
 * separated by a colon and wildcards.
 *
 * @param {String} namespaces
 * @api public
 */

function enable(namespaces) {
  exports.save(namespaces);

  exports.names = [];
  exports.skips = [];

  var split = (typeof namespaces === 'string' ? namespaces : '').split(/[\s,]+/);
  var len = split.length;

  for (var i = 0; i < len; i++) {
    if (!split[i]) continue; // ignore empty strings
    namespaces = split[i].replace(/\*/g, '.*?');
    if (namespaces[0] === '-') {
      exports.skips.push(new RegExp('^' + namespaces.substr(1) + '$'));
    } else {
      exports.names.push(new RegExp('^' + namespaces + '$'));
    }
  }
}

/**
 * Disable debug output.
 *
 * @api public
 */

function disable() {
  exports.enable('');
}

/**
 * Returns true if the given mode name is enabled, false otherwise.
 *
 * @param {String} name
 * @return {Boolean}
 * @api public
 */

function enabled(name) {
  var i, len;
  for (i = 0, len = exports.skips.length; i < len; i++) {
    if (exports.skips[i].test(name)) {
      return false;
    }
  }
  for (i = 0, len = exports.names.length; i < len; i++) {
    if (exports.names[i].test(name)) {
      return true;
    }
  }
  return false;
}

/**
 * Coerce `val`.
 *
 * @param {Mixed} val
 * @return {Mixed}
 * @api private
 */

function coerce(val) {
  if (val instanceof Error) return val.stack || val.message;
  return val;
}


/***/ },

/***/ "./node_modules/electron-squirrel-startup/node_modules/debug/src/index.js"
/*!********************************************************************************!*\
  !*** ./node_modules/electron-squirrel-startup/node_modules/debug/src/index.js ***!
  \********************************************************************************/
(module, __unused_webpack_exports, __webpack_require__) {

/**
 * Detect Electron renderer process, which is node, but we should
 * treat as a browser.
 */

if (typeof process !== 'undefined' && process.type === 'renderer') {
  module.exports = __webpack_require__(/*! ./browser.js */ "./node_modules/electron-squirrel-startup/node_modules/debug/src/browser.js");
} else {
  module.exports = __webpack_require__(/*! ./node.js */ "./node_modules/electron-squirrel-startup/node_modules/debug/src/node.js");
}


/***/ },

/***/ "./node_modules/electron-squirrel-startup/node_modules/debug/src/node.js"
/*!*******************************************************************************!*\
  !*** ./node_modules/electron-squirrel-startup/node_modules/debug/src/node.js ***!
  \*******************************************************************************/
(module, exports, __webpack_require__) {

/**
 * Module dependencies.
 */

var tty = __webpack_require__(/*! tty */ "tty");
var util = __webpack_require__(/*! util */ "util");

/**
 * This is the Node.js implementation of `debug()`.
 *
 * Expose `debug()` as the module.
 */

exports = module.exports = __webpack_require__(/*! ./debug */ "./node_modules/electron-squirrel-startup/node_modules/debug/src/debug.js");
exports.init = init;
exports.log = log;
exports.formatArgs = formatArgs;
exports.save = save;
exports.load = load;
exports.useColors = useColors;

/**
 * Colors.
 */

exports.colors = [6, 2, 3, 4, 5, 1];

/**
 * Build up the default `inspectOpts` object from the environment variables.
 *
 *   $ DEBUG_COLORS=no DEBUG_DEPTH=10 DEBUG_SHOW_HIDDEN=enabled node script.js
 */

exports.inspectOpts = Object.keys(process.env).filter(function (key) {
  return /^debug_/i.test(key);
}).reduce(function (obj, key) {
  // camel-case
  var prop = key
    .substring(6)
    .toLowerCase()
    .replace(/_([a-z])/g, function (_, k) { return k.toUpperCase() });

  // coerce string value into JS value
  var val = process.env[key];
  if (/^(yes|on|true|enabled)$/i.test(val)) val = true;
  else if (/^(no|off|false|disabled)$/i.test(val)) val = false;
  else if (val === 'null') val = null;
  else val = Number(val);

  obj[prop] = val;
  return obj;
}, {});

/**
 * The file descriptor to write the `debug()` calls to.
 * Set the `DEBUG_FD` env variable to override with another value. i.e.:
 *
 *   $ DEBUG_FD=3 node script.js 3>debug.log
 */

var fd = parseInt(process.env.DEBUG_FD, 10) || 2;

if (1 !== fd && 2 !== fd) {
  util.deprecate(function(){}, 'except for stderr(2) and stdout(1), any other usage of DEBUG_FD is deprecated. Override debug.log if you want to use a different log function (https://git.io/debug_fd)')()
}

var stream = 1 === fd ? process.stdout :
             2 === fd ? process.stderr :
             createWritableStdioStream(fd);

/**
 * Is stdout a TTY? Colored output is enabled when `true`.
 */

function useColors() {
  return 'colors' in exports.inspectOpts
    ? Boolean(exports.inspectOpts.colors)
    : tty.isatty(fd);
}

/**
 * Map %o to `util.inspect()`, all on a single line.
 */

exports.formatters.o = function(v) {
  this.inspectOpts.colors = this.useColors;
  return util.inspect(v, this.inspectOpts)
    .split('\n').map(function(str) {
      return str.trim()
    }).join(' ');
};

/**
 * Map %o to `util.inspect()`, allowing multiple lines if needed.
 */

exports.formatters.O = function(v) {
  this.inspectOpts.colors = this.useColors;
  return util.inspect(v, this.inspectOpts);
};

/**
 * Adds ANSI color escape codes if enabled.
 *
 * @api public
 */

function formatArgs(args) {
  var name = this.namespace;
  var useColors = this.useColors;

  if (useColors) {
    var c = this.color;
    var prefix = '  \u001b[3' + c + ';1m' + name + ' ' + '\u001b[0m';

    args[0] = prefix + args[0].split('\n').join('\n' + prefix);
    args.push('\u001b[3' + c + 'm+' + exports.humanize(this.diff) + '\u001b[0m');
  } else {
    args[0] = new Date().toUTCString()
      + ' ' + name + ' ' + args[0];
  }
}

/**
 * Invokes `util.format()` with the specified arguments and writes to `stream`.
 */

function log() {
  return stream.write(util.format.apply(util, arguments) + '\n');
}

/**
 * Save `namespaces`.
 *
 * @param {String} namespaces
 * @api private
 */

function save(namespaces) {
  if (null == namespaces) {
    // If you set a process.env field to null or undefined, it gets cast to the
    // string 'null' or 'undefined'. Just delete instead.
    delete process.env.DEBUG;
  } else {
    process.env.DEBUG = namespaces;
  }
}

/**
 * Load `namespaces`.
 *
 * @return {String} returns the previously persisted debug modes
 * @api private
 */

function load() {
  return process.env.DEBUG;
}

/**
 * Copied from `node/src/node.js`.
 *
 * XXX: It's lame that node doesn't expose this API out-of-the-box. It also
 * relies on the undocumented `tty_wrap.guessHandleType()` which is also lame.
 */

function createWritableStdioStream (fd) {
  var stream;
  var tty_wrap = process.binding('tty_wrap');

  // Note stream._type is used for test-module-load-list.js

  switch (tty_wrap.guessHandleType(fd)) {
    case 'TTY':
      stream = new tty.WriteStream(fd);
      stream._type = 'tty';

      // Hack to have stream not keep the event loop alive.
      // See https://github.com/joyent/node/issues/1726
      if (stream._handle && stream._handle.unref) {
        stream._handle.unref();
      }
      break;

    case 'FILE':
      var fs = __webpack_require__(/*! fs */ "fs");
      stream = new fs.SyncWriteStream(fd, { autoClose: false });
      stream._type = 'fs';
      break;

    case 'PIPE':
    case 'TCP':
      var net = __webpack_require__(/*! net */ "net");
      stream = new net.Socket({
        fd: fd,
        readable: false,
        writable: true
      });

      // FIXME Should probably have an option in net.Socket to create a
      // stream from an existing fd which is writable only. But for now
      // we'll just add this hack and set the `readable` member to false.
      // Test: ./node test/fixtures/echo.js < /etc/passwd
      stream.readable = false;
      stream.read = null;
      stream._type = 'pipe';

      // FIXME Hack to have stream not keep the event loop alive.
      // See https://github.com/joyent/node/issues/1726
      if (stream._handle && stream._handle.unref) {
        stream._handle.unref();
      }
      break;

    default:
      // Probably an error on in uv_guess_handle()
      throw new Error('Implement me. Unknown stream file type!');
  }

  // For supporting legacy API we put the FD here.
  stream.fd = fd;

  stream._isStdio = true;

  return stream;
}

/**
 * Init logic for `debug` instances.
 *
 * Create a new `inspectOpts` object in case `useColors` is set
 * differently for a particular `debug` instance.
 */

function init (debug) {
  debug.inspectOpts = {};

  var keys = Object.keys(exports.inspectOpts);
  for (var i = 0; i < keys.length; i++) {
    debug.inspectOpts[keys[i]] = exports.inspectOpts[keys[i]];
  }
}

/**
 * Enable namespaces listed in `process.env.DEBUG` initially.
 */

exports.enable(load());


/***/ },

/***/ "./node_modules/electron-squirrel-startup/node_modules/ms/index.js"
/*!*************************************************************************!*\
  !*** ./node_modules/electron-squirrel-startup/node_modules/ms/index.js ***!
  \*************************************************************************/
(module) {

/**
 * Helpers.
 */

var s = 1000;
var m = s * 60;
var h = m * 60;
var d = h * 24;
var y = d * 365.25;

/**
 * Parse or format the given `val`.
 *
 * Options:
 *
 *  - `long` verbose formatting [false]
 *
 * @param {String|Number} val
 * @param {Object} [options]
 * @throws {Error} throw an error if val is not a non-empty string or a number
 * @return {String|Number}
 * @api public
 */

module.exports = function(val, options) {
  options = options || {};
  var type = typeof val;
  if (type === 'string' && val.length > 0) {
    return parse(val);
  } else if (type === 'number' && isNaN(val) === false) {
    return options.long ? fmtLong(val) : fmtShort(val);
  }
  throw new Error(
    'val is not a non-empty string or a valid number. val=' +
      JSON.stringify(val)
  );
};

/**
 * Parse the given `str` and return milliseconds.
 *
 * @param {String} str
 * @return {Number}
 * @api private
 */

function parse(str) {
  str = String(str);
  if (str.length > 100) {
    return;
  }
  var match = /^((?:\d+)?\.?\d+) *(milliseconds?|msecs?|ms|seconds?|secs?|s|minutes?|mins?|m|hours?|hrs?|h|days?|d|years?|yrs?|y)?$/i.exec(
    str
  );
  if (!match) {
    return;
  }
  var n = parseFloat(match[1]);
  var type = (match[2] || 'ms').toLowerCase();
  switch (type) {
    case 'years':
    case 'year':
    case 'yrs':
    case 'yr':
    case 'y':
      return n * y;
    case 'days':
    case 'day':
    case 'd':
      return n * d;
    case 'hours':
    case 'hour':
    case 'hrs':
    case 'hr':
    case 'h':
      return n * h;
    case 'minutes':
    case 'minute':
    case 'mins':
    case 'min':
    case 'm':
      return n * m;
    case 'seconds':
    case 'second':
    case 'secs':
    case 'sec':
    case 's':
      return n * s;
    case 'milliseconds':
    case 'millisecond':
    case 'msecs':
    case 'msec':
    case 'ms':
      return n;
    default:
      return undefined;
  }
}

/**
 * Short format for `ms`.
 *
 * @param {Number} ms
 * @return {String}
 * @api private
 */

function fmtShort(ms) {
  if (ms >= d) {
    return Math.round(ms / d) + 'd';
  }
  if (ms >= h) {
    return Math.round(ms / h) + 'h';
  }
  if (ms >= m) {
    return Math.round(ms / m) + 'm';
  }
  if (ms >= s) {
    return Math.round(ms / s) + 's';
  }
  return ms + 'ms';
}

/**
 * Long format for `ms`.
 *
 * @param {Number} ms
 * @return {String}
 * @api private
 */

function fmtLong(ms) {
  return plural(ms, d, 'day') ||
    plural(ms, h, 'hour') ||
    plural(ms, m, 'minute') ||
    plural(ms, s, 'second') ||
    ms + ' ms';
}

/**
 * Pluralization helper.
 */

function plural(ms, n, name) {
  if (ms < n) {
    return;
  }
  if (ms < n * 1.5) {
    return Math.floor(ms / n) + ' ' + name;
  }
  return Math.ceil(ms / n) + ' ' + name + 's';
}


/***/ },

/***/ "./src/lib/app-profiler.js"
/*!*********************************!*\
  !*** ./src/lib/app-profiler.js ***!
  \*********************************/
(module, __unused_webpack_exports, __webpack_require__) {

const { execSync } = __webpack_require__(/*! node:child_process */ "node:child_process");
const path = __webpack_require__(/*! node:path */ "node:path");

const DEFAULT_APPS = [
  { key: 'Google Chrome', name: 'Google Chrome', kind: '浏览器', policy: '代理', group: '外网浏览' },
  { key: 'Chrome Helper', name: 'Chrome 辅助进程', kind: '浏览器', policy: '代理', group: '外网浏览' },
  { key: 'Safari', name: 'Safari', kind: '浏览器', policy: '直连', group: '国内浏览' },
  { key: 'WebKit', name: 'Safari 内核服务', kind: '浏览器', policy: '直连', group: '国内浏览' },
  { key: 'Doubao Browser', name: '豆包浏览器', kind: '浏览器', policy: '代理', group: '外网浏览' },
  { key: 'Doubao Browser Helper', name: '豆包浏览器辅助进程', kind: '浏览器', policy: '代理', group: '外网浏览' },
  { key: 'Telegram', name: 'Telegram', kind: '通讯', policy: '代理', group: '外网通讯' },
  { key: 'WeChat', name: '微信', kind: '通讯', policy: '直连', group: '国内通讯' },
  { key: 'WeChatAppEx', name: '微信小程序', kind: '通讯', policy: '直连', group: '国内通讯' },
  { key: 'QQ', name: 'QQ', kind: '通讯', policy: '直连', group: '国内通讯' },
  { key: 'QQ Helper', name: 'QQ 辅助进程', kind: '通讯', policy: '直连', group: '国内通讯' },
  { key: 'Claude', name: 'Claude', kind: 'AI', policy: '代理', group: '外网AI' },
  { key: 'ChatGPT', name: 'ChatGPT', kind: 'AI', policy: '代理', group: '外网AI' },
  { key: 'Kimi', name: 'Kimi', kind: 'AI', policy: '直连', group: '国内AI' },
  { key: 'Doubao', name: '豆包', kind: 'AI', policy: '直连', group: '国内AI' },
  { key: 'Qianwen', name: '通义千问', kind: 'AI', policy: '直连', group: '国内AI' },
  { key: 'Ollama', name: 'Ollama', kind: 'AI', policy: '直连', group: '本机AI' },
  { key: 'ToDesk', name: 'ToDesk', kind: '远程', policy: '直连', group: '远程控制' },
  { key: 'TencentMeeting', name: '腾讯会议', kind: '办公', policy: '直连', group: '国内办公' },
  { key: 'NeatDownloadManager', name: 'NDM 下载', kind: '工具', policy: '直连', group: '下载工具' },
  { key: 'WorkBuddy', name: '腾讯 WorkBuddy', kind: '办公', policy: '直连', group: '国内办公' },
  { key: 'FinalShell', name: 'FinalShell', kind: '开发', policy: '直连', group: '服务器工具' },
  { key: 'Obsidian', name: 'Obsidian', kind: '笔记', policy: '直连', group: '本机笔记' },
  { key: 'Codex', name: 'Codex', kind: 'AI', policy: '代理', group: '外网AI' }
];

class AppProfiler {
  constructor(store) {
    this.store = store;
  }

  getProfiles() {
    const policies = this.store.get('appPolicies') || {};
    const groups = this.store.get('appGroups') || {};
    const merged = DEFAULT_APPS.map((app) => ({
      ...app,
      policy: policies[app.key] || app.policy,
      group: groups[app.key] || app.group
    }));
    const known = new Set(DEFAULT_APPS.map((a) => a.key));
    for (const [key, policy] of Object.entries(policies)) {
      if (!known.has(key)) {
        merged.push({ key, name: key, kind: '自定义', policy, group: groups[key] || '自定义' });
      }
    }
    return merged;
  }

  setPolicy(appKey, policy) {
    const policies = this.store.get('appPolicies') || {};
    policies[appKey] = policy;
    this.store.set('appPolicies', policies);
    return this.getProfiles();
  }

  setGroup(appKey, groupName) {
    const groups = this.store.get('appGroups') || {};
    groups[appKey] = groupName;
    this.store.set('appGroups', groups);
    return this.getProfiles();
  }

  scanRunningApps() {
    const out = [];
    try {
      const text = execSync("ps -axo comm | sed 's#^/.*/##' | sort -u", { encoding: 'utf8', timeout: 3000 });
      const names = text.split('\n').map((s) => s.trim()).filter(Boolean);
      const profiles = this.getProfiles();
      for (const profile of profiles) {
        const hit = names.find((n) => n.toLowerCase().includes(profile.key.toLowerCase()));
        if (hit) out.push({ ...profile, running: true, processName: hit });
      }
    } catch (_) {}
    return out;
  }
}

module.exports = AppProfiler;


/***/ },

/***/ "./src/lib/proxy-engine.js"
/*!*********************************!*\
  !*** ./src/lib/proxy-engine.js ***!
  \*********************************/
(module, __unused_webpack_exports, __webpack_require__) {

const { spawn, spawnSync } = __webpack_require__(/*! node:child_process */ "node:child_process");
const fs = __webpack_require__(/*! node:fs */ "node:fs");
const path = __webpack_require__(/*! node:path */ "node:path");
const os = __webpack_require__(/*! node:os */ "node:os");
const net = __webpack_require__(/*! node:net */ "node:net");

const APP_NAME = '魔法代理';
const MIXED_PORT = 7897;
const API_PORT = 9098;

function defaultNodes() {
  return [
    {
      id: 'node-1',
      name: '搬瓦工直连',
      type: 'vless',
      server: '104.160.40.35',
      port: 443,
      uuid: '268a1166-d31e-478c-a66f-7f9c06c9afaa',
      flow: 'xtls-rprx-vision',
      servername: '',
      clientFingerprint: 'chrome',
      tls: true,
      udp: true,
      publicKey: 'c7wQJ08b7byOCBzPejQJSwTe8gVN5H4gZxY9vE-k1X0',
      shortId: 'be08e6123ddcaf32'
    },
    {
      id: 'node-2',
      name: 'Texas住宅',
      type: 'vless',
      server: '104.160.40.35',
      port: 443,
      uuid: '7f3e9a2b-4c5d-6e8f-1a2b-3c4d5e6f7a8b',
      flow: 'xtls-rprx-vision',
      servername: '',
      clientFingerprint: 'chrome',
      tls: true,
      udp: true,
      publicKey: 'c7wQJ08b7byOCBzPejQJSwTe8gVN5H4gZxY9vE-k1X0',
      shortId: 'bca4b7cfbcb66d57'
    }
  ];
}

class ProxyEngine {
  constructor(store, appProfiler, onLog) {
    this.store = store;
    this.appProfiler = appProfiler;
    this.onLog = onLog;
    this.child = null;
    this.ready = false;
    this.profileDir = path.join(os.homedir(), '.magic-agent', 'profiles');
    fs.mkdirSync(this.profileDir, { recursive: true });
  }

  log(message) {
    if (typeof this.onLog === 'function') this.onLog(message);
  }

  ensureBundledFiles() {
    const kernel = path.join(process.resourcesPath || '', 'bin', 'mihomo');
    if (fs.existsSync(kernel)) return kernel;
    const local = path.join(__dirname, '..', '..', 'bin', 'mihomo');
    if (fs.existsSync(local)) return local;
    this.log({ level: 'error', text: '未找到 mihomo 内核，请把 mihomo 放到 bin/mihomo' });
    return null;
  }

  _toNodeYaml(node) {
    const base = {
      name: node.name,
      type: node.type || 'vless',
      server: node.server,
      port: Number(node.port || 443),
      uuid: node.uuid || '',
      network: 'tcp',
      tls: true,
      udp: true,
      flow: node.flow || 'xtls-rprx-vision',
      servername: node.servername || '',
      'client-fingerprint': node.clientFingerprint || 'chrome'
    };
    if (node.publicKey && node.shortId) {
      base['reality-opts'] = {
        'public-key': node.publicKey,
        'short-id': node.shortId
      };
    }
    return base;
  }

  _buildConfig() {
    const state = this.store.get('proxy') || {};
    const nodes = (state.nodes && state.nodes.length ? state.nodes : defaultNodes()).map((n) => this._toNodeYaml(n));
    const groups = {
      name: 'GLOBAL',
      type: 'select',
      proxies: nodes.map((n) => n.name)
    };
    const policies = this.appProfiler ? this.appProfiler.getProfiles() : {};
    const rules = [];
    rules.push('IP-CIDR,127.0.0.0/8,DIRECT');
    rules.push('IP-CIDR,192.168.0.0/16,DIRECT');
    rules.push('IP-CIDR,10.0.0.0/8,DIRECT');
    rules.push('IP-CIDR,172.16.0.0/12,DIRECT');
    for (const [appKey, policy] of Object.entries(policies)) {
      if (!policy || policy.policy === '直连' || policy.policy === 'DIRECT') {
        rules.push(`PROCESS-NAME-REGEX,${escapeRegExp(appKey)},DIRECT`);
      }
    }
    if (state.mode === 'global') {
      rules.push('MATCH,GLOBAL');
    } else {
      rules.push('GEOSITE,cn,DIRECT');
      rules.push('GEOIP,CN,DIRECT');
      rules.push('MATCH,GLOBAL');
    }
    const config = {
      'mixed-port': MIXED_PORT,
      'allow-lan': false,
      bind: '127.0.0.1',
      mode: state.mode === 'global' ? 'global' : 'rule',
      'log-level': 'info',
      ipv6: false,
      'external-controller': `127.0.0.1:${API_PORT}`,
      'find-process-mode': 'always',
      proxies: nodes,
      'proxy-groups': [groups],
      rules
    };
    return config;
  }

  writeConfig() {
    const file = path.join(this.profileDir, 'config.yaml');
    fs.writeFileSync(file, yamlStringify(this._buildConfig()), 'utf8');
    return file;
  }

  snapshot() {
    const state = this.store.get('proxy') || {};
    return {
      enabled: this.ready,
      pid: this.child ? this.child.pid : null,
      mode: state.mode || 'auto',
      systemProxy: !!state.systemProxy,
      nodes: state.nodes && state.nodes.length ? state.nodes : defaultNodes(),
      selectedGroup: state.selectedGroup || 'GLOBAL',
      logs: this.getLogs().slice(-60)
    };
  }

  start() {
    if (this.ready) return this.snapshot();
    const kernel = this.ensureBundledFiles();
    if (!kernel) return this.snapshot();
    const file = this.writeConfig();
    this.log({ level: 'info', text: `启动 mihomo: ${file}` });
    this.child = spawn(kernel, ['-f', file], { stdio: ['ignore', 'pipe', 'pipe'] });
    this.child.stdout.on('data', (d) => this.log({ level: 'info', text: d.toString().trim() }));
    this.child.stderr.on('data', (d) => this.log({ level: 'error', text: d.toString().trim() }));
    this.child.on('exit', (code) => {
      this.ready = false;
      this.log({ level: 'warn', text: `mihomo 退出 code=${code}` });
      this.child = null;
    });
    return new Promise((resolve) => {
      const started = Date.now();
      const iv = setInterval(() => {
        const ok = net.connect({ host: '127.0.0.1', port: MIXED_PORT });
        ok.once('connect', () => {
          clearInterval(iv);
          ok.destroy();
          this.ready = true;
          this.store.set('proxy', { ...this.store.get('proxy'), enabled: true });
          this.log({ level: 'success', text: `代理已启动，端口 ${MIXED_PORT}` });
          resolve(this.snapshot());
        });
        ok.once('error', () => {
          if (Date.now() - started > 12000) {
            clearInterval(iv);
            resolve(this.snapshot());
          }
        });
      }, 400);
    });
  }

  stop() {
    if (this.child) {
      try { this.child.kill(); } catch (_) {}
      this.child = null;
    }
    this.ready = false;
    this.store.set('proxy', { ...this.store.get('proxy'), enabled: false });
    this.log({ level: 'info', text: '代理已停止' });
    return this.snapshot();
  }

  setMode(mode) {
    this.store.set('proxy', { ...this.store.get('proxy'), mode });
    this.log({ level: 'info', text: `分流模式：${mode === 'global' ? '全局' : '智能分流'}` });
    return this.snapshot();
  }

  addNode(node) {
    const state = this.store.get('proxy') || {};
    const nodes = state.nodes && state.nodes.length ? state.nodes : defaultNodes();
    nodes.push({ ...node, id: `node-${Date.now()}` });
    this.store.set('proxy', { ...state, nodes });
    return this.snapshot();
  }

  removeNode(index) {
    const state = this.store.get('proxy') || {};
    const nodes = (state.nodes || []).slice();
    nodes.splice(index, 1);
    this.store.set('proxy', { ...state, nodes });
    return this.snapshot();
  }

  importConfig(text) {
    try {
      const obj = JSON.parse(text);
      if (obj && Array.isArray(obj.nodes)) {
        this.store.set('proxy', { ...this.store.get('proxy'), nodes: obj.nodes });
        return this.snapshot();
      }
    } catch (_) {}
    this.store.set('importedConfig', text);
    return this.snapshot();
  }

  exportConfig() {
    return yamlStringify(this._buildConfig());
  }

  reloadConfig() {
    if (this.ready) {
      try { this.child && this.child.kill('SIGHUP'); } catch (_) {}
    }
    return this.snapshot();
  }

  getLogs() {
    return (this.store.get('proxyLogs') || []).map((x) => x);
  }

  init() {
    this.store.set('proxy', { ...this.store.get('proxy'), enabled: false, mode: this.store.get('proxy').mode || 'auto' });
    return this.snapshot();
  }
}

function escapeRegExp(s) {
  return String(s).replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function yamlStringify(obj) {
  return 'mixed-port: 7897\nallow-lan: false\nbind-address: 127.0.0.1\nmode: "' + (obj.mode || 'rule') + '"\nlog-level: info\nipv6: false\nexternal-controller: 127.0.0.1:9098\nfind-process-mode: always\nproxies:\n' +
    (obj.proxies || []).map((p) => {
      const lines = ['  - name: "' + p.name + '"', '    type: ' + p.type, '    server: ' + p.server, '    port: ' + p.port, '    uuid: "' + p.uuid + '"', '    network: tcp', '    tls: true', '    udp: true', '    flow: "' + (p.flow || '') + '"', '    servername: "' + (p.servername || '') + '"', '    client-fingerprint: "' + (p['client-fingerprint'] || 'chrome') + '"'];
      if (p['reality-opts']) lines.push('    reality-opts:\n      public-key: "' + p['reality-opts']['public-key'] + '"\n      short-id: "' + p['reality-opts']['short-id'] + '"');
      return lines.join('\n');
    }).join('\n') +
    '\nproxy-groups:\n  - name: GLOBAL\n    type: select\n    proxies:\n' + (obj.proxies || []).map((p) => '      - "' + p.name + '"').join('\n') +
    '\nrules:\n' + (obj.rules || []).map((r) => '  - ' + r).join('\n') + '\n';
}

module.exports = ProxyEngine;


/***/ },

/***/ "./src/lib/server-manager.js"
/*!***********************************!*\
  !*** ./src/lib/server-manager.js ***!
  \***********************************/
(module, __unused_webpack_exports, __webpack_require__) {

const { spawn } = __webpack_require__(/*! node:child_process */ "node:child_process");
const fs = __webpack_require__(/*! node:fs */ "node:fs");
const path = __webpack_require__(/*! node:path */ "node:path");
const os = __webpack_require__(/*! node:os */ "node:os");

class ServerManager {
  constructor(store, onLog) {
    this.store = store;
    this.onLog = onLog;
    this.sessions = new Map();
    this.config = store.get('serverConfig') || {};
  }

  log(msg) {
    if (typeof this.onLog === 'function') this.onLog(msg);
  }

  _servers() {
    return this.store.get('servers') || [];
  }

  snapshot() {
    return {
      servers: this._servers().map((s) => {
        const live = this.sessions.get(s.id);
        return { ...s, connected: !!live && live.connected, lastEvent: live ? live.lastEvent : null };
      }),
      config: this.config
    };
  }

  addServer(server) {
    const servers = this._servers().slice();
    const id = server.id || `srv-${Date.now()}`;
    servers.push({ ...server, id });
    this.store.set('servers', servers);
    this.log({ level: 'success', text: `已添加服务器 ${server.name || id}` });
    return this.snapshot();
  }

  updateServer(id, patch) {
    const servers = this._servers().map((s) => (s.id === id ? { ...s, ...patch, id } : s));
    this.store.set('servers', servers);
    return this.snapshot();
  }

  removeServer(id) {
    this.disconnect(id);
    this.store.set('servers', this._servers().filter((s) => s.id !== id));
    return this.snapshot();
  }

  connect(id) {
    const server = this._servers().find((s) => s.id === id);
    if (!server) throw new Error('服务器不存在');
    const args = [
      '-p', String(server.port || 22),
      '-o', 'StrictHostKeyChecking=no',
      '-o', 'UserKnownHostsFile=/dev/null',
      '-o', 'ServerAliveInterval=15',
      '-o', 'ServerAliveCountMax=3'
    ];
    if (server.privateKey) args.push('-i', server.privateKey);
    const sshTarget = `${server.username}@${server.host}`;
    this.log({ level: 'info', text: `连接 ${server.name} (${sshTarget}:${server.port || 22})` });
    const child = spawn('ssh', args.concat([sshTarget]), { stdio: ['pipe', 'pipe', 'pipe'] });
    const session = { child, connected: false, buffer: '', lastEvent: 'connecting', type: server.type || 'ssh' };
    this.sessions.set(id, session);
    child.stdout.on('data', (d) => {
      session.buffer = (session.buffer + d.toString()).slice(-8000);
      session.lastEvent = 'output';
      this._emit(id, 'output', d.toString());
    });
    child.stderr.on('data', (d) => {
      session.buffer = (session.buffer + d.toString()).slice(-8000);
      const s = d.toString();
      if (/password:|password for/i.test(s)) session.lastEvent = 'need-password';
      if (/WARNING: REMOTE HOST IDENTIFICATION/i.test(s)) session.lastEvent = 'host-key';
      if (/Enter passphrase/i.test(s)) session.lastEvent = 'need-passphrase';
      this._emit(id, 'stderr', s);
    });
    child.on('exit', (code) => {
      session.connected = false;
      session.lastEvent = `exit-${code}`;
      this._emit(id, 'exit', `退出 code=${code}`);
      this.sessions.delete(id);
    });
    setTimeout(() => {
      if (this.sessions.has(id) && !session.connected && session.buffer.length === 0) {
        session.connected = true;
        session.lastEvent = 'connected';
        this._emit(id, 'connected', '已连接');
      }
    }, 600);
    return this.snapshot();
  }

  disconnect(id) {
    const session = this.sessions.get(id);
    if (session && session.child) {
      try { session.child.kill('SIGHUP'); } catch (_) {}
      session.connected = false;
      session.lastEvent = 'disconnected';
      this._emit(id, 'disconnected', '已断开');
    }
    this.sessions.delete(id);
    return this.snapshot();
  }

  write(id, data) {
    const session = this.sessions.get(id);
    if (session && session.child && session.child.stdin.writable) {
      session.child.stdin.write(data);
      session.lastEvent = 'input';
    }
    return true;
  }

  resize(id, cols, rows) {
    const session = this.sessions.get(id);
    if (session && session.child && session.child.stdout.setEncoding) {
      try { session.child.stdout.setEncoding('utf8'); } catch (_) {}
    }
    return true;
  }

  setConfig(cfg) {
    this.config = cfg || {};
    this.store.set('serverConfig', this.config);
    return this.snapshot();
  }

  _emit(id, type, data) {
    const cb = this.config.onEvent;
    if (typeof cb === 'function') {
      try { cb({ id, type, data }); } catch (_) {}
    }
    this.log({ level: 'info', text: `[${type}] ${String(data).slice(0, 120)}` });
  }

  dispose() {
    for (const id of Array.from(this.sessions.keys())) this.disconnect(id);
  }

  init() {
    return this.snapshot();
  }
}

module.exports = ServerManager;


/***/ },

/***/ "./src/lib/store.js"
/*!**************************!*\
  !*** ./src/lib/store.js ***!
  \**************************/
(module, __unused_webpack_exports, __webpack_require__) {

const fs = __webpack_require__(/*! node:fs */ "node:fs");
const path = __webpack_require__(/*! node:path */ "node:path");
const os = __webpack_require__(/*! node:os */ "node:os");
const { app } = __webpack_require__(/*! electron */ "electron");

class Store {
  constructor() {
    this.dir = app ? path.join(app.getPath('userData'), 'data') : path.join(os.homedir(), '.magic-agent');
    fs.mkdirSync(this.dir, { recursive: true });
    this.file = path.join(this.dir, 'state.json');
    this.data = this.load();
  }

  load() {
    try {
      return JSON.parse(fs.readFileSync(this.file, 'utf8'));
    } catch {
      return {
        proxy: { enabled: false, mode: 'auto', selectedGroup: '🚀 智能分流', nodes: [], systemProxy: true },
        appPolicies: {},
        appGroups: {},
        servers: [],
        serverConfig: {},
        importedConfig: ''
      };
    }
  }

  save() {
    try {
      fs.mkdirSync(path.dirname(this.file), { recursive: true });
      fs.writeFileSync(this.file, JSON.stringify(this.data, null, 2));
    } catch (err) {
      console.error('Store save error', err);
    }
  }

  get(key) {
    return this.data[key];
  }

  set(key, value) {
    this.data[key] = value;
    this.save();
    return value;
  }
}

module.exports = Store;


/***/ },

/***/ "child_process"
/*!********************************!*\
  !*** external "child_process" ***!
  \********************************/
(module) {

"use strict";
module.exports = require("child_process");

/***/ },

/***/ "electron"
/*!***************************!*\
  !*** external "electron" ***!
  \***************************/
(module) {

"use strict";
module.exports = require("electron");

/***/ },

/***/ "fs"
/*!*********************!*\
  !*** external "fs" ***!
  \*********************/
(module) {

"use strict";
module.exports = require("fs");

/***/ },

/***/ "net"
/*!**********************!*\
  !*** external "net" ***!
  \**********************/
(module) {

"use strict";
module.exports = require("net");

/***/ },

/***/ "node:child_process"
/*!*************************************!*\
  !*** external "node:child_process" ***!
  \*************************************/
(module) {

"use strict";
module.exports = require("node:child_process");

/***/ },

/***/ "node:fs"
/*!**************************!*\
  !*** external "node:fs" ***!
  \**************************/
(module) {

"use strict";
module.exports = require("node:fs");

/***/ },

/***/ "node:net"
/*!***************************!*\
  !*** external "node:net" ***!
  \***************************/
(module) {

"use strict";
module.exports = require("node:net");

/***/ },

/***/ "node:os"
/*!**************************!*\
  !*** external "node:os" ***!
  \**************************/
(module) {

"use strict";
module.exports = require("node:os");

/***/ },

/***/ "node:path"
/*!****************************!*\
  !*** external "node:path" ***!
  \****************************/
(module) {

"use strict";
module.exports = require("node:path");

/***/ },

/***/ "path"
/*!***********************!*\
  !*** external "path" ***!
  \***********************/
(module) {

"use strict";
module.exports = require("path");

/***/ },

/***/ "tty"
/*!**********************!*\
  !*** external "tty" ***!
  \**********************/
(module) {

"use strict";
module.exports = require("tty");

/***/ },

/***/ "util"
/*!***********************!*\
  !*** external "util" ***!
  \***********************/
(module) {

"use strict";
module.exports = require("util");

/***/ }

/******/ 	});
/************************************************************************/
/******/ 	// The module cache
/******/ 	const __webpack_module_cache__ = {};
/******/ 	
/******/ 	// The require function
/******/ 	function __webpack_require__(moduleId) {
/******/ 		// Check if module is in cache
/******/ 		const cachedModule = __webpack_module_cache__[moduleId];
/******/ 		if (cachedModule !== undefined) {
/******/ 			return cachedModule.exports;
/******/ 		}
/******/ 		// Create a new module (and put it into the cache)
/******/ 		const module = __webpack_module_cache__[moduleId] = {
/******/ 			// no module.id needed
/******/ 			// no module.loaded needed
/******/ 			exports: {}
/******/ 		};
/******/ 	
/******/ 		// Execute the module function
/******/ 		if (!(moduleId in __webpack_modules__)) {
/******/ 			delete __webpack_module_cache__[moduleId];
/******/ 			const e = new Error("Cannot find module '" + moduleId + "'");
/******/ 			e.code = 'MODULE_NOT_FOUND';
/******/ 			throw e;
/******/ 		}
/******/ 		__webpack_modules__[moduleId](module, module.exports, __webpack_require__);
/******/ 	
/******/ 		// Return the exports of the module
/******/ 		return module.exports;
/******/ 	}
/******/ 	
/************************************************************************/
/******/ 	/* webpack/runtime/compat */
/******/ 	
/******/ 	if (typeof __webpack_require__ !== 'undefined') __webpack_require__.ab = __dirname + "/native_modules/";
/******/ 	
/************************************************************************/
let __webpack_exports__ = {};
// This entry needs to be wrapped in an IIFE because it needs to be isolated against other modules in the chunk.
(() => {
/*!*********************!*\
  !*** ./src/main.js ***!
  \*********************/
const { app, BrowserWindow, ipcMain, shell } = __webpack_require__(/*! electron */ "electron");
const path = __webpack_require__(/*! node:path */ "node:path");
const fs = __webpack_require__(/*! node:fs */ "node:fs");
const os = __webpack_require__(/*! node:os */ "node:os");
const { execFile } = __webpack_require__(/*! node:child_process */ "node:child_process");

const ProxyEngine = __webpack_require__(/*! ./lib/proxy-engine */ "./src/lib/proxy-engine.js");
const ServerManager = __webpack_require__(/*! ./lib/server-manager */ "./src/lib/server-manager.js");
const AppProfiler = __webpack_require__(/*! ./lib/app-profiler */ "./src/lib/app-profiler.js");
const Store = __webpack_require__(/*! ./lib/store */ "./src/lib/store.js");

let mainWindow = null;
const store = new Store();
const appProfiler = new AppProfiler(store);
const serverManager = new ServerManager(store, (log) => pushLog(log));
const proxyEngine = new ProxyEngine(store, appProfiler, (log) => pushLog(log));

if (__webpack_require__(/*! electron-squirrel-startup */ "./node_modules/electron-squirrel-startup/index.js")) {
  app.quit();
}

function pushLog(message) {
  if (mainWindow && !mainWindow.isDestroyed()) {
    mainWindow.webContents.send('magic:log', message);
  }
}

function createWindow() {
  mainWindow = new BrowserWindow({
    width: 1240,
    height: 800,
    minWidth: 1080,
    minHeight: 680,
    title: '魔法代理',
    backgroundColor: '#f4f6fb',
    titleBarStyle: 'hiddenInset',
    trafficLightPosition: { x: 18, y: 18 },
    webPreferences: {
      preload: '/Users/zhangxuetao/Desktop/魔法代理/.webpack/renderer/main_window/preload.js',
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: false
    }
  });

  mainWindow.loadURL('http://localhost:3000/main_window/index.html');
  mainWindow.on('closed', () => { mainWindow = null; });
  mainWindow.webContents.setWindowOpenHandler(({ url }) => {
    if (url.startsWith('http://') || url.startsWith('https://')) shell.openExternal(url);
    return { action: 'deny' };
  });
}

app.whenReady().then(async () => {
  try {
    proxyEngine.ensureBundledFiles();
    await proxyEngine.init();
    await serverManager.init();
  } catch (err) {
    pushLog({ level: 'error', text: `初始化失败: ${err.message}` });
  }
  createWindow();
});

app.on('activate', () => {
  if (BrowserWindow.getAllWindows().length === 0) createWindow();
});

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') app.quit();
});

app.on('before-quit', async () => {
  try {
    await proxyEngine.stop();
    serverManager.dispose();
  } catch (_) {}
});

ipcMain.handle('magic:get-state', async () => {
  const proxy = await proxyEngine.snapshot();
  const servers = serverManager.snapshot();
  const config = store.get('appConfig') || {};
  return {
    proxy,
    servers,
    config,
    appProfiles: appProfiler.getProfiles(),
    runningApps: appProfiler.scanRunningApps()
  };
});

ipcMain.handle('magic:start-proxy', async () => {
  await proxyEngine.start();
  return proxyEngine.snapshot();
});

ipcMain.handle('magic:stop-proxy', async () => {
  await proxyEngine.stop();
  return proxyEngine.snapshot();
});

ipcMain.handle('magic:set-proxy-mode', async (_event, mode) => {
  await proxyEngine.setMode(mode);
  return proxyEngine.snapshot();
});

ipcMain.handle('magic:set-app-policy', async (_event, appKey, policy) => {
  await appProfiler.setPolicy(appKey, policy);
  await proxyEngine.reloadConfig();
  return { profiles: appProfiler.getProfiles(), proxy: await proxyEngine.snapshot() };
});

ipcMain.handle('magic:set-app-group', async (_event, appKey, groupName) => {
  await appProfiler.setGroup(appKey, groupName);
  await proxyEngine.reloadConfig();
  return { profiles: appProfiler.getProfiles(), proxy: await proxyEngine.snapshot() };
});

ipcMain.handle('magic:add-node', async (_event, node) => {
  await proxyEngine.addNode(node);
  return proxyEngine.snapshot();
});

ipcMain.handle('magic:remove-node', async (_event, index) => {
  await proxyEngine.removeNode(index);
  return proxyEngine.snapshot();
});

ipcMain.handle('magic:import-config', async (_event, text) => {
  await proxyEngine.importConfig(text);
  return proxyEngine.snapshot();
});

ipcMain.handle('magic:export-config', async () => {
  return { text: proxyEngine.exportConfig() };
});

ipcMain.handle('magic:add-server', async (_event, server) => {
  await serverManager.addServer(server);
  return serverManager.snapshot();
});

ipcMain.handle('magic:update-server', async (_event, id, patch) => {
  await serverManager.updateServer(id, patch);
  return serverManager.snapshot();
});

ipcMain.handle('magic:remove-server', async (_event, id) => {
  await serverManager.removeServer(id);
  return serverManager.snapshot();
});

ipcMain.handle('magic:connect-server', async (_event, id) => {
  await serverManager.connect(id);
  return serverManager.snapshot();
});

ipcMain.handle('magic:disconnect-server', async (_event, id) => {
  await serverManager.disconnect(id);
  return serverManager.snapshot();
});

ipcMain.handle('magic:server-write', async (_event, id, data) => {
  serverManager.write(id, data);
  return true;
});

ipcMain.handle('magic:server-resize', async (_event, id, cols, rows) => {
  serverManager.resize(id, cols, rows);
  return true;
});

ipcMain.handle('magic:pick-server', async () => {
  if (!mainWindow) return null;
  const result = await (__webpack_require__(/*! node:child_process */ "node:child_process").execFile)(
    '/usr/bin/security', ['find-generic-password', '-s', '魔法代理SSH'], { timeout: 3000 },
    (err) => { void err; }
  );
  void result;
  return null;
});

ipcMain.handle('magic:set-server-config', async (_event, cfg) => {
  serverManager.setConfig(cfg || {});
  return serverManager.snapshot();
});

ipcMain.handle('magic:get-log', () => proxyEngine.getLogs());

ipcMain.handle('magic:select-config-file', async () => {
  const { dialog } = __webpack_require__(/*! electron */ "electron");
  if (!mainWindow) return null;
  const r = await dialog.showOpenDialog(mainWindow, {
    title: '选择 Clash / mihomo 配置',
    properties: ['openFile'],
    filters: [{ name: 'YAML', extensions: ['yaml', 'yml'] }]
  });
  if (r.canceled || !r.filePaths[0]) return null;
  const text = fs.readFileSync(r.filePaths[0], 'utf8');
  await proxyEngine.importConfig(text);
  return proxyEngine.snapshot();
});

})();

module.exports = __webpack_exports__;
/******/ })()
;
//# sourceMappingURL=index.js.map
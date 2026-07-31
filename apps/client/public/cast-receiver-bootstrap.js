/**
 * Starts the Google Cast receiver framework before the application bundle runs.
 *
 * A Cast sender keeps its launch request pending until the receiver calls
 * CastReceiverContext.start(). The Euripus bundle is far too large to boot
 * within that window on Chromecast hardware, so the framework is started here
 * instead and the application publishes its pairing status once it is ready.
 *
 * Kept dependency-free and ES5-only so it parses on the oldest Cast devices.
 */
(function () {
  var NAMESPACE = "urn:x-cast:se.olivermarcusson.euripus.receiver";
  var SDK_URL =
    "https://www.gstatic.com/cast/sdk/libs/caf_receiver/v3/cast_receiver_framework.js";
  var SDK_LOAD_ATTEMPTS = 3;
  var SDK_RETRY_DELAY_MS = 1000;
  var ANNOUNCE_INTERVAL_MS = 2000;
  var ANNOUNCE_ATTEMPTS = 30;

  function isCastReceiver() {
    return (
      /[?&]cast=1(?:&|$)/.test(window.location.search) ||
      /CrKey|GoogleTV/i.test(window.navigator.userAgent)
    );
  }

  if (!isCastReceiver()) {
    return;
  }

  var receiver = {
    namespace: NAMESPACE,
    context: null,
    status: null,
    started: false,
    failed: false,
    publish: publish,
  };
  window.__euripusCastReceiver = receiver;

  var acknowledged = false;
  var announceTimer = null;
  var announcesLeft = 0;

  function announce() {
    if (!receiver.context || !receiver.status) {
      return;
    }
    try {
      receiver.context.sendCustomMessage(NAMESPACE, undefined, receiver.status);
    } catch (error) {
      // A sender may disconnect between the check and the send.
    }
  }

  function stopAnnouncing() {
    if (announceTimer !== null) {
      window.clearInterval(announceTimer);
      announceTimer = null;
    }
  }

  /**
   * Custom Cast messages are not buffered, so a status published while no
   * sender is attached is simply dropped. Repeat until a sender acknowledges.
   */
  function startAnnouncing() {
    announcesLeft = ANNOUNCE_ATTEMPTS;
    announce();
    if (announceTimer !== null) {
      return;
    }
    announceTimer = window.setInterval(function () {
      announcesLeft -= 1;
      if (acknowledged || announcesLeft <= 0) {
        stopAnnouncing();
        return;
      }
      announce();
    }, ANNOUNCE_INTERVAL_MS);
  }

  function publish(status) {
    receiver.status = status;
    acknowledged = false;
    startAnnouncing();
  }

  function handleSenderMessage(event) {
    var message = event && event.data;
    if (typeof message === "string") {
      try {
        message = JSON.parse(message);
      } catch (error) {
        return;
      }
    }
    if (!message || typeof message !== "object") {
      return;
    }
    if (message.type === "request_receiver_status") {
      acknowledged = false;
      startAnnouncing();
      return;
    }
    if (message.type === "receiver_status_ack") {
      acknowledged = true;
      stopAnnouncing();
    }
  }

  function start() {
    var framework = window.cast && window.cast.framework;
    if (!framework) {
      receiver.failed = true;
      return;
    }

    try {
      var context = framework.CastReceiverContext.getInstance();
      // CAF only routes custom namespaces registered before start().
      context.addCustomMessageListener(NAMESPACE, handleSenderMessage);
      context.addEventListener(
        framework.system.EventType.SENDER_CONNECTED,
        function () {
          acknowledged = false;
          startAnnouncing();
        },
      );
      context.start({ disableIdleTimeout: true, statusText: "Euripus Receiver" });
      receiver.context = context;
      receiver.started = true;
      if (receiver.status) {
        startAnnouncing();
      }
    } catch (error) {
      receiver.failed = true;
    }
  }

  function loadSdk(attemptsLeft) {
    if (window.cast && window.cast.framework) {
      start();
      return;
    }

    var script = document.createElement("script");
    script.src = SDK_URL;
    script.onload = start;
    script.onerror = function () {
      if (script.parentNode) {
        script.parentNode.removeChild(script);
      }
      if (attemptsLeft <= 1) {
        receiver.failed = true;
        return;
      }
      window.setTimeout(function () {
        loadSdk(attemptsLeft - 1);
      }, SDK_RETRY_DELAY_MS);
    };
    (document.head || document.documentElement).appendChild(script);
  }

  loadSdk(SDK_LOAD_ATTEMPTS);
})();

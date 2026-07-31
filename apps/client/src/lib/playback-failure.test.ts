import { mediaErrorFailureReason } from "@/lib/plyr-player";
import { describeReceiverFailure } from "@/features/receiver/receiver-page";

const MEDIA_ERR_ABORTED = 1;
const MEDIA_ERR_NETWORK = 2;
const MEDIA_ERR_DECODE = 3;
const MEDIA_ERR_SRC_NOT_SUPPORTED = 4;

describe("media element failure classification", () => {
  it("routes decode and unsupported-source failures to the transcode fallback", () => {
    expect(mediaErrorFailureReason({ code: MEDIA_ERR_DECODE })).toBe("codec");
    expect(mediaErrorFailureReason({ code: MEDIA_ERR_SRC_NOT_SUPPORTED })).toBe(
      "codec",
    );
  });

  it("keeps transport failures out of the codec path", () => {
    expect(mediaErrorFailureReason({ code: MEDIA_ERR_NETWORK })).toBe("network");
    expect(mediaErrorFailureReason({ code: MEDIA_ERR_ABORTED })).toBe(
      "video-error",
    );
    expect(mediaErrorFailureReason(null)).toBe("video-error");
  });
});

describe("receiver failure messages", () => {
  it("only blames the video format when the format is actually the problem", () => {
    expect(
      describeReceiverFailure({ kind: "recoverable", reason: "codec" }),
    ).toBe("This stream's video format is not supported by this Cast device.");
    expect(
      describeReceiverFailure({ kind: "recoverable", reason: "network" }),
    ).toBe("The receiver lost connection to this stream.");
    expect(describeReceiverFailure({ kind: "recoverable", reason: "hls" })).toBe(
      "This stream could not be played on the receiver.",
    );
  });

  it("passes provider messages through unchanged", () => {
    expect(
      describeReceiverFailure({
        kind: "provider-unavailable",
        message: "This channel is currently unavailable from the provider.",
      }),
    ).toBe("This channel is currently unavailable from the provider.");
  });
});

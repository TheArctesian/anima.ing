#import <ApplicationServices/ApplicationServices.h>
#import <CoreMedia/CoreMedia.h>
#import <CoreVideo/CoreVideo.h>
#import <ScreenCaptureKit/ScreenCaptureKit.h>

#include <stdint.h>

typedef void (*AnimaFrame)(void *, const uint8_t *, size_t, size_t, size_t);
typedef void (*AnimaRelease)(void *);

@interface AnimaDisplayCapture : NSObject <SCStreamOutput> {
    AnimaFrame _receive;
    AnimaRelease _release;
    void *_context;
}
@property(nonatomic, strong) SCStream *stream;
@property(nonatomic, strong) dispatch_queue_t frames;
- (instancetype)initWithReceiver:(AnimaFrame)receive
                         release:(AnimaRelease)release
                         context:(void *)context;
@end

@implementation AnimaDisplayCapture
- (instancetype)initWithReceiver:(AnimaFrame)receive
                         release:(AnimaRelease)release
                         context:(void *)context {
    self = [super init];
    if (self) {
        _receive = receive;
        _release = release;
        _context = context;
        _frames = dispatch_queue_create("ing.anima.rdp.frames", DISPATCH_QUEUE_SERIAL);
    }
    return self;
}

- (void)dealloc {
    if (_release) _release(_context);
}

- (void)stream:(SCStream *)stream
    didOutputSampleBuffer:(CMSampleBufferRef)sample
    ofType:(SCStreamOutputType)type {
    if (type != SCStreamOutputTypeScreen || !CMSampleBufferIsValid(sample)) return;
    CVPixelBufferRef pixel = CMSampleBufferGetImageBuffer(sample);
    if (!pixel || CVPixelBufferIsPlanar(pixel) ||
        CVPixelBufferGetPixelFormatType(pixel) != kCVPixelFormatType_32BGRA) return;
    if (CVPixelBufferLockBaseAddress(pixel, kCVPixelBufferLock_ReadOnly) != kCVReturnSuccess) return;
    const void *base = CVPixelBufferGetBaseAddress(pixel);
    size_t stride = CVPixelBufferGetBytesPerRow(pixel);
    size_t height = CVPixelBufferGetHeight(pixel);
    if (base && stride && height <= CVPixelBufferGetDataSize(pixel) / stride) {
        _receive(_context, base, CVPixelBufferGetWidth(pixel), height, stride);
    }
    CVPixelBufferUnlockBaseAddress(pixel, kCVPixelBufferLock_ReadOnly);
}
@end

static dispatch_time_t limit(void) {
    return dispatch_time(DISPATCH_TIME_NOW, 5 * NSEC_PER_SEC);
}

uint8_t anima_rdp_permissions(void) {
    return (CGPreflightScreenCaptureAccess() ? 1 : 0) |
           (CGPreflightPostEventAccess() ? 2 : 0);
}

uint8_t anima_rdp_request_permissions(void) {
    if (!CGPreflightScreenCaptureAccess()) CGRequestScreenCaptureAccess();
    if (!CGPreflightPostEventAccess()) CGRequestPostEventAccess();
    return anima_rdp_permissions();
}

// The caller transfers context ownership, also when starting fails.
void *anima_rdp_capture(AnimaFrame receive, AnimaRelease release, void *context,
                        uint32_t display_id, uint16_t width, uint16_t height) {
    @autoreleasepool {
        AnimaDisplayCapture *capture = [[AnimaDisplayCapture alloc] initWithReceiver:receive
                                                                             release:release
                                                                             context:context];
        if ((anima_rdp_permissions() & 1) == 0) return NULL;
        dispatch_semaphore_t found = dispatch_semaphore_create(0);
        __block SCDisplay *display = nil;
        [SCShareableContent getShareableContentWithCompletionHandler:^(SCShareableContent *content, NSError *error) {
            if (!error) {
                for (SCDisplay *candidate in content.displays) {
                    if (candidate.displayID == display_id) {
                        display = candidate;
                        break;
                    }
                }
            }
            dispatch_semaphore_signal(found);
        }];
        if (dispatch_semaphore_wait(found, limit()) || !display) return NULL;

        SCStreamConfiguration *config = [[SCStreamConfiguration alloc] init];
        config.width = width;
        config.height = height;
        config.pixelFormat = kCVPixelFormatType_32BGRA;
        config.minimumFrameInterval = CMTimeMake(1, 10);
        config.queueDepth = 3;
        config.showsCursor = YES;
        config.colorSpaceName = kCGColorSpaceSRGB;
        SCContentFilter *filter = [[SCContentFilter alloc] initWithDisplay:display excludingWindows:@[]];
        capture.stream = [[SCStream alloc] initWithFilter:filter configuration:config delegate:nil];
        if (![capture.stream addStreamOutput:capture type:SCStreamOutputTypeScreen
                           sampleHandlerQueue:capture.frames error:NULL]) return NULL;
        dispatch_semaphore_t started = dispatch_semaphore_create(0);
        __block BOOL start_failed = NO;
        [capture.stream startCaptureWithCompletionHandler:^(NSError *error) {
            start_failed = error != nil;
            dispatch_semaphore_signal(started);
        }];
        if (dispatch_semaphore_wait(started, limit()) || start_failed) {
            [capture.stream removeStreamOutput:capture type:SCStreamOutputTypeScreen error:NULL];
            [capture.stream stopCaptureWithCompletionHandler:nil];
            return NULL;
        }
        return (__bridge_retained void *)capture;
    }
}

void anima_rdp_stop_capture(void *reference) {
    @autoreleasepool {
        AnimaDisplayCapture *capture = CFBridgingRelease(reference);
        [capture.stream removeStreamOutput:capture type:SCStreamOutputTypeScreen error:NULL];
        [capture.stream stopCaptureWithCompletionHandler:nil];
        dispatch_sync(capture.frames, ^{});
    }
}

uint32_t anima_rdp_main_display(void) {
    return CGMainDisplayID();
}

uint32_t anima_rdp_display_width(uint32_t display) {
    return (uint32_t)CGDisplayPixelsWide(display);
}

uint32_t anima_rdp_display_height(uint32_t display) {
    return (uint32_t)CGDisplayPixelsHigh(display);
}

void anima_rdp_key(uint16_t code, bool down, uint64_t flags, uint16_t unit) {
    @autoreleasepool {
        if (!CGPreflightPostEventAccess()) return;
        CGEventRef event = CGEventCreateKeyboardEvent(NULL, code, down);
        if (!event) return;
        CGEventSetFlags(event, flags);
        if (unit) CGEventKeyboardSetUnicodeString(event, 1, &unit);
        CGEventPost(kCGHIDEventTap, event);
        CFRelease(event);
    }
}

void anima_rdp_pointer(uint32_t display, double x, double y, uint32_t type,
                       uint32_t button, uint64_t flags) {
    @autoreleasepool {
        if (!CGPreflightPostEventAccess()) return;
        CGRect bounds = CGDisplayBounds(display);
        if (CGRectIsEmpty(bounds)) return;
        CGPoint point = CGPointMake(bounds.origin.x + bounds.size.width * x,
                                    bounds.origin.y + bounds.size.height * y);
        CGEventRef event = CGEventCreateMouseEvent(NULL, type, point, button);
        if (!event) return;
        CGEventSetFlags(event, flags);
        CGEventPost(kCGHIDEventTap, event);
        CFRelease(event);
    }
}

void anima_rdp_scroll(int32_t x, int32_t y, uint64_t flags) {
    @autoreleasepool {
        if (!CGPreflightPostEventAccess()) return;
        CGEventRef event = CGEventCreateScrollWheelEvent(NULL, kCGScrollEventUnitLine, 2, y, x);
        if (!event) return;
        CGEventSetFlags(event, flags);
        CGEventPost(kCGHIDEventTap, event);
        CFRelease(event);
    }
}

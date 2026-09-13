#import <AppKit/AppKit.h>
#import <Photos/Photos.h>
#import <ImageIO/ImageIO.h>
#import <stdatomic.h>

static atomic_uint cancellationGeneration;

void reveal_photos_cancel(void) {
    atomic_fetch_add(&cancellationGeneration, 1);
}

static NSDictionary *failure(NSString *message) {
    return @{@"error": message ?: @"Apple Photos request failed"};
}

static BOOL waitFor(dispatch_semaphore_t semaphore, unsigned generation, int seconds) {
    for (int i = 0; i < seconds * 10; i++) {
        if (dispatch_semaphore_wait(semaphore, dispatch_time(DISPATCH_TIME_NOW, NSEC_PER_SEC / 10)) == 0)
            return YES;
        if (atomic_load(&cancellationGeneration) != generation) return NO;
    }
    return NO;
}

static NSString *authorizationName(PHAuthorizationStatus status) {
    switch (status) {
        case PHAuthorizationStatusAuthorized: return @"authorized";
        case PHAuthorizationStatusLimited: return @"limited";
        case PHAuthorizationStatusDenied: return @"denied";
        case PHAuthorizationStatusRestricted: return @"restricted";
        default: return @"notDetermined";
    }
}

static PHAssetResource *originalResource(PHAsset *asset) {
    NSArray<PHAssetResource *> *resources = [PHAssetResource assetResourcesForAsset:asset];
    NSSet *rawExtensions = [NSSet setWithArray:@[@"raf", @"dng", @"nef", @"arw", @"cr2", @"cr3", @"orf", @"rw2", @"pef", @"srw"]];
    for (PHAssetResource *resource in resources) {
        if ((resource.type == PHAssetResourceTypePhoto || resource.type == PHAssetResourceTypeAlternatePhoto) &&
            [rawExtensions containsObject:resource.originalFilename.pathExtension.lowercaseString])
            return resource;
    }
    for (PHAssetResource *resource in resources) {
        if (resource.type == PHAssetResourceTypePhoto) return resource;
    }
    return nil;
}

static NSDictionary *assetInfo(PHAsset *asset) {
    PHAssetResource *resource = originalResource(asset);
    return @{
        @"id": asset.localIdentifier,
        @"name": resource.originalFilename ?: @"Photo",
        @"width": @(asset.pixelWidth), @"height": @(asset.pixelHeight),
        @"capture_at": asset.creationDate ? @((long long)asset.creationDate.timeIntervalSince1970) : [NSNull null],
        @"revision": asset.modificationDate ? @(asset.modificationDate.timeIntervalSince1970) : @0
    };
}

static PHFetchOptions *photoOptions(BOOL descending) {
    PHFetchOptions *options = [PHFetchOptions new];
    options.includeHiddenAssets = NO;
    options.predicate = [NSPredicate predicateWithFormat:@"mediaType == %d AND hidden == NO", PHAssetMediaTypeImage];
    options.sortDescriptors = @[[NSSortDescriptor sortDescriptorWithKey:@"creationDate" ascending:!descending]];
    return options;
}

// Only public, still-photo smart collections are exposed. In particular, do
// not surface Hidden, Recently Deleted, video-only or unknown private types.
static BOOL browsableAlbum(PHAssetCollection *album) {
    if (album.assetCollectionType != PHAssetCollectionTypeSmartAlbum) return YES;
    switch (album.assetCollectionSubtype) {
        case PHAssetCollectionSubtypeSmartAlbumGeneric:
        case PHAssetCollectionSubtypeSmartAlbumPanoramas:
        case PHAssetCollectionSubtypeSmartAlbumFavorites:
        case PHAssetCollectionSubtypeSmartAlbumRecentlyAdded:
        case PHAssetCollectionSubtypeSmartAlbumBursts:
        case PHAssetCollectionSubtypeSmartAlbumUserLibrary:
        case PHAssetCollectionSubtypeSmartAlbumSelfPortraits:
        case PHAssetCollectionSubtypeSmartAlbumScreenshots:
        case PHAssetCollectionSubtypeSmartAlbumDepthEffect:
        case PHAssetCollectionSubtypeSmartAlbumLivePhotos:
        case PHAssetCollectionSubtypeSmartAlbumAnimated:
        case PHAssetCollectionSubtypeSmartAlbumLongExposures:
        case PHAssetCollectionSubtypeSmartAlbumUnableToUpload:
            return YES;
        default:
            if (@available(macOS 12, *))
                return album.assetCollectionSubtype == PHAssetCollectionSubtypeSmartAlbumRAW;
            return NO;
    }
}

static NSArray *collectionTree(PHFetchResult<PHCollection *> *collections, PHFetchOptions *options) {
    NSMutableArray *nodes = [NSMutableArray array];
    for (PHCollection *collection in collections) {
        if ([collection isKindOfClass:PHCollectionList.class]) {
            PHFetchResult *children = [PHCollection fetchCollectionsInCollectionList:(PHCollectionList *)collection options:nil];
            [nodes addObject:@{
                @"id": collection.localIdentifier, @"title": collection.localizedTitle ?: @"Folder",
                @"kind": @"folder", @"count": NSNull.null, @"children": collectionTree(children, options)
            }];
        } else if ([collection isKindOfClass:PHAssetCollection.class]) {
            PHAssetCollection *album = (PHAssetCollection *)collection;
            if (!browsableAlbum(album)) continue;
            NSUInteger count = [PHAsset fetchAssetsInAssetCollection:album options:options].count;
            // Keep empty user albums/folders, but omit irrelevant smart albums.
            if (album.assetCollectionType == PHAssetCollectionTypeSmartAlbum && !count) continue;
            [nodes addObject:@{
                @"id": collection.localIdentifier, @"title": collection.localizedTitle ?: @"Album",
                @"kind": @"album", @"count": @(count), @"children": @[]
            }];
        }
    }
    return nodes;
}

// Folder selection mirrors recursive filesystem browsing. Assets belonging to
// several descendant albums appear only once; no originals are downloaded.
static void collectFolderAssetIDs(PHCollectionList *folder, PHFetchOptions *options, NSMutableSet<NSString *> *identifiers) {
    for (PHCollection *child in [PHCollection fetchCollectionsInCollectionList:folder options:nil]) {
        if ([child isKindOfClass:PHCollectionList.class]) {
            collectFolderAssetIDs((PHCollectionList *)child, options, identifiers);
        } else if ([child isKindOfClass:PHAssetCollection.class] && browsableAlbum((PHAssetCollection *)child)) {
            for (PHAsset *asset in [PHAsset fetchAssetsInAssetCollection:(PHAssetCollection *)child options:options])
                [identifiers addObject:asset.localIdentifier];
        }
    }
}

// Convert rendered formats (including HEIC) to an oriented, color-managed sRGB
// TIFF. RAW resources bypass this path and retain their original sensor data.
static BOOL writeSRGBTIFF(NSURL *sourceURL, NSURL *destinationURL, NSError **error) {
    CGImageSourceRef source = CGImageSourceCreateWithURL((__bridge CFURLRef)sourceURL, NULL);
    if (!source) return NO;
    NSDictionary *properties = CFBridgingRelease(CGImageSourceCopyPropertiesAtIndex(source, 0, NULL));
    NSUInteger edge = MAX([properties[(id)kCGImagePropertyPixelWidth] unsignedIntegerValue],
                          [properties[(id)kCGImagePropertyPixelHeight] unsignedIntegerValue]);
    NSDictionary *options = @{
        (id)kCGImageSourceCreateThumbnailFromImageAlways: @YES,
        (id)kCGImageSourceCreateThumbnailWithTransform: @YES,
        (id)kCGImageSourceThumbnailMaxPixelSize: @(MAX(edge, 1))
    };
    CGImageRef image = CGImageSourceCreateThumbnailAtIndex(source, 0, (__bridge CFDictionaryRef)options);
    CFRelease(source);
    if (!image) return NO;
    size_t width = CGImageGetWidth(image), height = CGImageGetHeight(image);
    CGColorSpaceRef space = CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
    CGContextRef context = CGBitmapContextCreate(NULL, width, height, 8, 0, space, kCGImageAlphaPremultipliedLast);
    CGColorSpaceRelease(space);
    if (!context) { CGImageRelease(image); return NO; }
    CGContextDrawImage(context, CGRectMake(0, 0, width, height), image);
    CGImageRelease(image);
    CGImageRef converted = CGBitmapContextCreateImage(context);
    CGContextRelease(context);
    if (!converted) return NO;
    CGImageDestinationRef destination = CGImageDestinationCreateWithURL((__bridge CFURLRef)destinationURL, CFSTR("public.tiff"), 1, NULL);
    BOOL success = NO;
    if (destination) {
        CGImageDestinationAddImage(destination, converted, NULL);
        success = CGImageDestinationFinalize(destination);
        CFRelease(destination);
    }
    CGImageRelease(converted);
    if (!success && error) *error = [NSError errorWithDomain:@"RevealPhotos" code:1 userInfo:@{NSLocalizedDescriptionKey: @"Could not create the sRGB working image"}];
    return success;
}

int reveal_photos_convert(const char *source, const char *destination) {
    @autoreleasepool {
        return writeSRGBTIFF(
            [NSURL fileURLWithPath:[NSString stringWithUTF8String:source]],
            [NSURL fileURLWithPath:[NSString stringWithUTF8String:destination]], NULL);
    }
}

static NSDictionary *performRequest(NSDictionary *request) {
    NSString *operation = request[@"operation"];
    unsigned generation = atomic_load(&cancellationGeneration);
    PHAuthorizationStatus status = [PHPhotoLibrary authorizationStatusForAccessLevel:PHAccessLevelReadWrite];
    if ([operation isEqualToString:@"status"])
        return @{@"authorization": authorizationName(status)};
    if ([operation isEqualToString:@"authorize"]) {
        if (status == PHAuthorizationStatusNotDetermined) {
            dispatch_semaphore_t semaphore = dispatch_semaphore_create(0);
            __block PHAuthorizationStatus result = status;
            [PHPhotoLibrary requestAuthorizationForAccessLevel:PHAccessLevelReadWrite handler:^(PHAuthorizationStatus value) {
                result = value;
                dispatch_semaphore_signal(semaphore);
            }];
            if (!waitFor(semaphore, generation, 120)) return failure(@"Photo library permission request cancelled or timed out");
            status = result;
        }
        return @{@"authorization": authorizationName(status)};
    }
    if (status != PHAuthorizationStatusAuthorized && status != PHAuthorizationStatusLimited)
        return failure(@"Allow Reveal access in System Settings > Privacy & Security > Photos, then reconnect.");

    if ([operation isEqualToString:@"albums"]) {
        PHFetchOptions *options = photoOptions(NO);
        NSMutableArray *albums = [collectionTree([PHCollection fetchTopLevelUserCollectionsWithOptions:nil], options) mutableCopy];
        PHFetchResult *smartAlbums = [PHAssetCollection fetchAssetCollectionsWithType:PHAssetCollectionTypeSmartAlbum subtype:PHAssetCollectionSubtypeAny options:nil];
        // The catalogue header already opens the entire library.
        NSMutableArray *smartNodes = [collectionTree(smartAlbums, options) mutableCopy];
        PHAssetCollection *library = [PHAssetCollection fetchAssetCollectionsWithType:PHAssetCollectionTypeSmartAlbum subtype:PHAssetCollectionSubtypeSmartAlbumUserLibrary options:nil].firstObject;
        if (library) [smartNodes filterUsingPredicate:[NSPredicate predicateWithFormat:@"id != %@", library.localIdentifier]];
        [albums addObjectsFromArray:smartNodes];
        return @{@"albums": albums, @"total": @([PHAsset fetchAssetsWithOptions:options].count)};
    }
    PHFetchOptions *options = photoOptions([request[@"descending"] boolValue]);
    if ([operation isEqualToString:@"list"]) {
        PHFetchResult<PHAsset *> *assets;
        NSString *album = request[@"album"];
        if (album.length) {
            PHAssetCollection *collection = [PHAssetCollection fetchAssetCollectionsWithLocalIdentifiers:@[album] options:nil].firstObject;
            if (collection) {
                if (!browsableAlbum(collection)) return failure(@"This album is not available in Reveal");
                assets = [PHAsset fetchAssetsInAssetCollection:collection options:options];
            } else {
                PHCollectionList *folder = [PHCollectionList fetchCollectionListsWithLocalIdentifiers:@[album] options:nil].firstObject;
                if (!folder) return failure(@"This album or folder is no longer available");
                NSMutableSet *identifiers = [NSMutableSet set];
                collectFolderAssetIDs(folder, options, identifiers);
                assets = [PHAsset fetchAssetsWithLocalIdentifiers:identifiers.allObjects options:options];
            }
        } else {
            assets = [PHAsset fetchAssetsWithOptions:options];
        }
        NSUInteger offset = MIN([request[@"offset"] unsignedIntegerValue], assets.count);
        NSUInteger count = MIN(200, assets.count - offset);
        NSMutableArray *rows = [NSMutableArray arrayWithCapacity:count];
        for (NSUInteger i = offset; i < offset + count; i++) [rows addObject:assetInfo(assets[i])];
        return @{@"frames": rows, @"total": @(assets.count), @"next": @(offset + count)};
    }
    NSString *identifier = request[@"id"];
    if (!identifier.length) return failure(@"Missing Apple Photos asset identifier");
    PHAsset *asset = [PHAsset fetchAssetsWithLocalIdentifiers:@[identifier] options:options].firstObject;
    if (!asset) return failure(@"This photo was removed, is hidden, or is no longer accessible");
    if ([operation isEqualToString:@"info"]) return assetInfo(asset);

    if ([operation isEqualToString:@"thumbnail"]) {
        PHImageRequestOptions *imageOptions = [PHImageRequestOptions new];
        imageOptions.networkAccessAllowed = YES;
        imageOptions.deliveryMode = PHImageRequestOptionsDeliveryModeHighQualityFormat;
        imageOptions.resizeMode = PHImageRequestOptionsResizeModeFast;
        imageOptions.version = PHImageRequestOptionsVersionOriginal;
        dispatch_semaphore_t semaphore = dispatch_semaphore_create(0);
        __block NSImage *result;
        __block NSError *imageError;
        NSUInteger size = MIN(MAX([request[@"size"] unsignedIntegerValue], 256), 2560);
        PHImageRequestID imageRequest = [[PHImageManager defaultManager] requestImageForAsset:asset targetSize:CGSizeMake(size, size) contentMode:PHImageContentModeAspectFit options:imageOptions resultHandler:^(NSImage *image, NSDictionary *info) {
            if ([info[PHImageResultIsDegradedKey] boolValue]) return;
            result = image;
            imageError = info[PHImageErrorKey];
            dispatch_semaphore_signal(semaphore);
        }];
        if (!waitFor(semaphore, generation, 60)) {
            [[PHImageManager defaultManager] cancelImageRequest:imageRequest];
            return failure(@"Photo preview cancelled or timed out. Check your iCloud connection and retry.");
        }
        if (!result) return failure(imageError.localizedDescription ?: @"The photo preview is unavailable");
        CGImageRef cgImage = [result CGImageForProposedRect:NULL context:nil hints:nil];
        if (!cgImage) return failure(@"Could not decode the photo preview");
        NSBitmapImageRep *bitmap = [[NSBitmapImageRep alloc] initWithCGImage:cgImage];
        NSData *jpeg = [bitmap representationUsingType:NSBitmapImageFileTypeJPEG properties:@{NSImageCompressionFactor: @0.9}];
        NSError *error;
        if (!jpeg || ![jpeg writeToFile:request[@"destination"] options:NSDataWritingAtomic error:&error])
            return failure(error.localizedDescription ?: @"Could not cache the photo preview");
        return @{};
    }
    if ([operation isEqualToString:@"source"]) {
        PHAssetResource *resource = originalResource(asset);
        if (!resource) return failure(@"No still-image original is available for this photo");
        NSString *directory = request[@"directory"];
        NSString *ext = resource.originalFilename.pathExtension.lowercaseString;
        NSSet *rawExtensions = [NSSet setWithArray:@[@"raf", @"dng", @"nef", @"arw", @"cr2", @"cr3", @"orf", @"rw2", @"pef", @"srw"]];
        BOOL raw = [rawExtensions containsObject:ext];
        NSString *filename = raw ? [@"original." stringByAppendingString:ext] : @"working.tiff";
        NSString *finalPath = [directory stringByAppendingPathComponent:filename];
        NSString *temporary = [directory stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
        PHAssetResourceRequestOptions *resourceOptions = [PHAssetResourceRequestOptions new];
        resourceOptions.networkAccessAllowed = YES;
        dispatch_semaphore_t semaphore = dispatch_semaphore_create(0);
        __block NSError *downloadError;
        [[NSFileManager defaultManager] createFileAtPath:temporary contents:nil attributes:nil];
        NSError *openError;
        NSFileHandle *file = [NSFileHandle fileHandleForWritingToURL:[NSURL fileURLWithPath:temporary] error:&openError];
        if (!file) return failure(openError.localizedDescription);
        NSLock *lock = [NSLock new];
        __block BOOL closed = NO;
        PHAssetResourceDataRequestID download = [[PHAssetResourceManager defaultManager] requestDataForAssetResource:resource options:resourceOptions dataReceivedHandler:^(NSData *data) {
            [lock lock];
            if (!closed && !downloadError) {
                NSError *writeError;
                if (![file writeData:data error:&writeError]) downloadError = writeError;
            }
            [lock unlock];
        } completionHandler:^(NSError *error) {
            [lock lock];
            if (error) downloadError = error;
            [lock unlock];
            dispatch_semaphore_signal(semaphore);
        }];
        BOOL completed = waitFor(semaphore, generation, 300);
        if (!completed) [[PHAssetResourceManager defaultManager] cancelDataRequest:download];
        [lock lock];
        closed = YES;
        NSError *closeError;
        [file closeAndReturnError:&closeError];
        NSError *error = downloadError ?: closeError;
        [lock unlock];
        if (!completed || error) {
            [[NSFileManager defaultManager] removeItemAtPath:temporary error:nil];
            return failure(error.localizedDescription ?: @"Photo download cancelled or timed out. Check your iCloud connection and retry.");
        }
        if (!raw) {
            NSString *converted = [temporary stringByAppendingString:@".tiff"];
            BOOL ok = writeSRGBTIFF([NSURL fileURLWithPath:temporary], [NSURL fileURLWithPath:converted], &error);
            [[NSFileManager defaultManager] removeItemAtPath:temporary error:nil];
            if (!ok) {
                [[NSFileManager defaultManager] removeItemAtPath:converted error:nil];
                return failure(error.localizedDescription ?: @"Unsupported photo format");
            }
            temporary = converted;
        }
        if (![[NSFileManager defaultManager] moveItemAtPath:temporary toPath:finalPath error:&error]) {
            [[NSFileManager defaultManager] removeItemAtPath:temporary error:nil];
            return failure(error.localizedDescription);
        }
        return @{@"filename": filename};
    }
    return failure(@"Unknown Apple Photos operation");
}

char *reveal_photos_request(const char *json) {
    @autoreleasepool {
        NSError *error;
        NSData *data = [[NSString stringWithUTF8String:json] dataUsingEncoding:NSUTF8StringEncoding];
        NSDictionary *request = [NSJSONSerialization JSONObjectWithData:data options:0 error:&error];
        NSDictionary *response = request ? performRequest(request) : failure(error.localizedDescription);
        NSData *encoded = [NSJSONSerialization dataWithJSONObject:response options:0 error:&error];
        return strdup(encoded ? [[NSString alloc] initWithData:encoded encoding:NSUTF8StringEncoding].UTF8String : "{\"error\":\"Could not encode PhotoKit response\"}");
    }
}

void reveal_photos_free(char *response) { free(response); }

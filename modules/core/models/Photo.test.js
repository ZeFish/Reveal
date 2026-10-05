import { describe, it, expect } from "vitest";
import { Photo } from "./Photo.js";

describe("Photo Model", () => {
  const sampleFrame = {
    path: "/Volumes/Photos/2026/09/DSCF1234.RAF",
    name: "DSCF1234.RAF",
    previewVersion: 3,
    rating: 4,
    width: 6000,
    height: 4000,
    aperture: 2.8,
    shutter: "1/500s",
    iso: 800,
    focal_mm: 33,
    make: "FUJIFILM",
    model: "X-T5",
  };

  describe("static Photo.thumb", () => {
    it("generates correct reveal://thumb URL from object", () => {
      const url = Photo.thumb(sampleFrame);
      expect(url).toBe("reveal://thumb?p=%2FVolumes%2FPhotos%2F2026%2F09%2FDSCF1234.RAF&v=3&size=768");
    });

    it("generates correct URL from raw path string with defaults", () => {
      const url = Photo.thumb("/a/b.jpg");
      expect(url).toBe("reveal://thumb?p=%2Fa%2Fb.jpg&v=0&size=768");
    });

    it("respects custom size, version, attempt and priority options", () => {
      const url = Photo.thumb(sampleFrame, { size: 2048, version: 10, attempt: 2, priority: true });
      expect(url).toBe(
        "reveal://thumb?p=%2FVolumes%2FPhotos%2F2026%2F09%2FDSCF1234.RAF&v=10&size=2048&r=2&priority=1"
      );
    });

    it("handles null or undefined safely", () => {
      expect(Photo.thumb(null)).toBe("");
      expect(Photo.thumb(undefined)).toBe("");
    });
  });

  describe("static Photo.stem", () => {
    it("extracts stem from filename and path", () => {
      expect(Photo.stem("/dir/photo.jpg")).toBe("photo");
      expect(Photo.stem("photo.RAF")).toBe("photo");
      expect(Photo.stem(sampleFrame)).toBe("DSCF1234");
    });

    it("handles .reveal.jpg intermediate extensions", () => {
      expect(Photo.stem("2026/09/IMG_4012.reveal.jpg")).toBe("IMG_4012");
    });

    it("handles filenames without extensions or empty inputs", () => {
      expect(Photo.stem("untitled")).toBe("untitled");
      expect(Photo.stem("")).toBe("");
      expect(Photo.stem(null)).toBe("");
    });
  });

  describe("static Photo.isRaw", () => {
    it("identifies RAW camera file formats", () => {
      expect(Photo.isRaw("DSCF0001.RAF")).toBe(true);
      expect(Photo.isRaw("/Volumes/SD/IMG_0001.CR3")).toBe(true);
      expect(Photo.isRaw("photo.NEF")).toBe(true);
      expect(Photo.isRaw("photo.ARW")).toBe(true);
      expect(Photo.isRaw("photo.DNG")).toBe(true);
    });

    it("identifies non-RAW file formats", () => {
      expect(Photo.isRaw("photo.jpg")).toBe(false);
      expect(Photo.isRaw("photo.jpeg")).toBe(false);
      expect(Photo.isRaw("photo.png")).toBe(false);
      expect(Photo.isRaw("photo.webp")).toBe(false);
      expect(Photo.isRaw("photo.tiff")).toBe(false);
      expect(Photo.isRaw(null)).toBe(false);
    });
  });

  describe("static Photo.aspectRatio", () => {
    it("computes ratio when width and height are available", () => {
      expect(Photo.aspectRatio(sampleFrame)).toBe(1.5);
    });

    it("falls back to default ratio when missing dimensions", () => {
      expect(Photo.aspectRatio({ path: "/a/b.jpg" })).toBe(1.5);
      expect(Photo.aspectRatio(null, 1.33)).toBe(1.33);
    });
  });

  describe("static Photo.formatExposure", () => {
    it("formats full EXIF exposure line", () => {
      expect(Photo.formatExposure(sampleFrame)).toBe("ƒ2.8 · 1/500s · ISO 800 · 33mm");
    });

    it("formats partial EXIF gracefully", () => {
      expect(Photo.formatExposure({ aperture: 1.4, iso: 100 })).toBe("ƒ1.4 · ISO 100");
      expect(Photo.formatExposure(null)).toBe("");
    });
  });

  describe("OOP instance", () => {
    it("creates an instance via Photo.from", () => {
      const photo = Photo.from(sampleFrame);
      expect(photo).toBeInstanceOf(Photo);
      expect(photo?.path).toBe(sampleFrame.path);
      expect(photo?.stem).toBe("DSCF1234");
      expect(photo?.isRaw).toBe(true);
      expect(photo?.aspectRatio).toBe(1.5);
      expect(photo?.exposure).toBe("ƒ2.8 · 1/500s · ISO 800 · 33mm");
      expect(photo?.thumb).toBe("reveal://thumb?p=%2FVolumes%2FPhotos%2F2026%2F09%2FDSCF1234.RAF&v=3&size=768");
      expect(photo?.largeThumb).toBe("reveal://thumb?p=%2FVolumes%2FPhotos%2F2026%2F09%2FDSCF1234.RAF&v=3&size=2048");
    });

    it("returns existing instance when passing a Photo to Photo.from", () => {
      const p1 = new Photo(sampleFrame);
      const p2 = Photo.from(p1);
      expect(p1).toBe(p2);
    });

    it("returns null when passing null or undefined to Photo.from", () => {
      expect(Photo.from(null)).toBeNull();
      expect(Photo.from(undefined)).toBeNull();
    });
  });
});

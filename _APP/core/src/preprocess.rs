// ==============================================================================
// SCRIPT: preprocess.rs (txtmelhorator-core)
// DESCRIÇÃO: Pré-processamento leve de imagem antes do OCR (R2 depois)
// CHAMADO POR: extraction OCR path (opt-in)
// CONTRATO (RESPOSTA ESPERADA): bitmap mais legível; sem inventar pixels de texto
// ==============================================================================

use image::{DynamicImage, GrayImage, Luma};

/// Escala de cinza + contraste simples (stretch) — deskew fica para depois.
pub fn prepare_for_ocr(img: &DynamicImage) -> GrayImage {
    let gray = img.to_luma8();
    let (w, h) = gray.dimensions();
    let mut min_v = 255u8;
    let mut max_v = 0u8;
    for p in gray.pixels() {
        min_v = min_v.min(p.0[0]);
        max_v = max_v.max(p.0[0]);
    }
    if max_v <= min_v {
        return gray;
    }
    let span = (max_v - min_v) as f32;
    let mut out = GrayImage::new(w, h);
    for (x, y, p) in gray.enumerate_pixels() {
        let v = ((p.0[0] - min_v) as f32 / span * 255.0).round() as u8;
        out.put_pixel(x, y, Luma([v]));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Luma};

    #[test]
    fn contraste_nao_muda_tamanho() {
        let img: GrayImage = ImageBuffer::from_fn(8, 8, |x, y| Luma([((x + y) * 10) as u8]));
        let out = prepare_for_ocr(&DynamicImage::ImageLuma8(img));
        assert_eq!(out.dimensions(), (8, 8));
    }
}

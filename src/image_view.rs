use std::sync::Arc;

use vulkano::device::{Device, DeviceOwned};
use vulkano::format::{Format, FormatFeatures};
use vulkano::image::view::{ImageView, ImageViewCreationError, ImageViewType};
use vulkano::image::{
    AttachmentImage, ImageAccess, ImageDescriptorLayouts, ImageDimensions, ImageInner, ImageLayout,
    ImageSubresourceRange, ImageUsage, ImageViewAbstract, StorageImage,
};
use vulkano::sampler::ycbcr::SamplerYcbcrConversion;
use vulkano::sampler::ComponentMapping;
use vulkano::VulkanObject;

#[derive(Debug)]
pub enum ImtImageVarient {
    Storage(Arc<StorageImage>),
    Attachment(Arc<AttachmentImage>),
}

#[derive(Debug)]
pub struct ImtImageView {
    view: Arc<ImageView<ImtImageVarient>>,
}

impl ImtImageView {
    /// Create a `ImtImageView` from a vulkano `StorageImage`.
    pub fn from_storage(image: Arc<StorageImage>) -> Result<Arc<Self>, ImageViewCreationError> {
        Ok(Arc::new(Self {
            view: ImageView::new_default(Arc::new(ImtImageVarient::Storage(image)))?,
        }))
    }

    /// Create a `ImtImageView` from a vulkano `AttachmentImage`.
    pub fn from_attachment(
        image: Arc<AttachmentImage>,
    ) -> Result<Arc<Self>, ImageViewCreationError> {
        Ok(Arc::new(Self {
            view: ImageView::new_default(Arc::new(ImtImageVarient::Attachment(image)))?,
        }))
    }

    #[inline]
    pub fn image_view_ref(&self) -> &ImageView<ImtImageVarient> {
        &self.view
    }

    /// Fetch the dimensions of this image.
    #[inline]
    pub fn dimensions(&self) -> ImageDimensions {
        self.image_view_ref().image().dimensions()
    }
}

unsafe impl ImageAccess for ImtImageVarient {
    #[inline]
    fn inner(&self) -> ImageInner<'_> {
        match self {
            Self::Storage(i) => i.inner(),
            Self::Attachment(i) => i.inner(),
        }
    }

    #[inline]
    fn initial_layout_requirement(&self) -> ImageLayout {
        match self {
            Self::Storage(i) => i.initial_layout_requirement(),
            Self::Attachment(i) => i.initial_layout_requirement(),
        }
    }

    #[inline]
    fn final_layout_requirement(&self) -> ImageLayout {
        match self {
            Self::Storage(i) => i.final_layout_requirement(),
            Self::Attachment(i) => i.final_layout_requirement(),
        }
    }

    #[inline]
    fn descriptor_layouts(&self) -> Option<ImageDescriptorLayouts> {
        match self {
            Self::Storage(i) => i.descriptor_layouts(),
            Self::Attachment(i) => i.descriptor_layouts(),
        }
    }

    #[inline]
    unsafe fn layout_initialized(&self) {
        match self {
            Self::Storage(i) => i.layout_initialized(),
            Self::Attachment(i) => i.layout_initialized(),
        }
    }

    #[inline]
    fn is_layout_initialized(&self) -> bool {
        match self {
            Self::Storage(i) => i.is_layout_initialized(),
            Self::Attachment(i) => i.is_layout_initialized(),
        }
    }
}

unsafe impl DeviceOwned for ImtImageVarient {
    fn device(&self) -> &Arc<Device> {
        match self {
            Self::Storage(i) => i.device(),
            Self::Attachment(i) => i.device(),
        }
    }
}

unsafe impl ImageViewAbstract for ImtImageView {
    #[inline]
    fn image(&self) -> Arc<dyn ImageAccess> {
        self.image_view_ref().image().clone() as Arc<dyn ImageAccess>
    }

    #[inline]
    fn component_mapping(&self) -> ComponentMapping {
        self.image_view_ref().component_mapping()
    }

    #[inline]
    fn filter_cubic(&self) -> bool {
        self.image_view_ref().filter_cubic()
    }

    #[inline]
    fn filter_cubic_minmax(&self) -> bool {
        self.image_view_ref().filter_cubic_minmax()
    }

    #[inline]
    fn format(&self) -> Option<Format> {
        self.image_view_ref().format()
    }

    #[inline]
    fn format_features(&self) -> &FormatFeatures {
        self.image_view_ref().format_features()
    }

    #[inline]
    fn sampler_ycbcr_conversion(&self) -> Option<&Arc<SamplerYcbcrConversion>> {
        self.image_view_ref().sampler_ycbcr_conversion()
    }

    #[inline]
    fn subresource_range(&self) -> &ImageSubresourceRange {
        self.image_view_ref().subresource_range()
    }

    #[inline]
    fn usage(&self) -> &ImageUsage {
        self.image_view_ref().usage()
    }

    #[inline]
    fn view_type(&self) -> ImageViewType {
        self.image_view_ref().view_type()
    }
}

unsafe impl VulkanObject for ImtImageView {
    type Object = ash::vk::ImageView;

    #[inline]
    fn internal_object(&self) -> ash::vk::ImageView {
        self.image_view_ref().internal_object()
    }
}

unsafe impl DeviceOwned for ImtImageView {
    fn device(&self) -> &Arc<Device> {
        self.image_view_ref().device()
    }
}

impl PartialEq for ImtImageView {
    fn eq(&self, other: &Self) -> bool {
        self.image_view_ref() == other.image_view_ref()
    }
}

impl Eq for ImtImageView {}

unsafe impl ImageAccess for ImtImageView {
    #[inline]
    fn inner(&self) -> ImageInner<'_> {
        self.image_view_ref().image().inner()
    }

    #[inline]
    fn initial_layout_requirement(&self) -> ImageLayout {
        self.image_view_ref().image().initial_layout_requirement()
    }

    #[inline]
    fn final_layout_requirement(&self) -> ImageLayout {
        self.image_view_ref().image().final_layout_requirement()
    }

    #[inline]
    fn descriptor_layouts(&self) -> Option<ImageDescriptorLayouts> {
        self.image_view_ref().image().descriptor_layouts()
    }

    #[inline]
    unsafe fn layout_initialized(&self) {
        self.image_view_ref().image().layout_initialized()
    }

    #[inline]
    fn is_layout_initialized(&self) -> bool {
        self.image_view_ref().image().is_layout_initialized()
    }
}

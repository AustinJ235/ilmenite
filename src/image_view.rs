use std::sync::Arc;
use vulkano::device::{Device, DeviceOwned};
use vulkano::format::{Format, FormatFeatures};
use vulkano::image::view::{ImageView, ImageViewCreationError, ImageViewType};
use vulkano::image::{
    ImageAccess, ImageDescriptorLayouts, ImageDimensions, ImageInner, ImageLayout,
    ImageSubresourceRange, ImageUsage, ImageViewAbstract, StorageImage,
};
use vulkano::sampler::ycbcr::SamplerYcbcrConversion;
use vulkano::sampler::ComponentMapping;
use vulkano::VulkanObject;

pub struct ImtImageView {
    view: Arc<ImageView<StorageImage>>,
}

impl ImtImageView {
    pub(crate) fn new(image: Arc<StorageImage>) -> Result<Arc<Self>, ImageViewCreationError> {
        Ok(Arc::new(Self {
            view: ImageView::new_default(image)?,
        }))
    }

    #[inline]
    pub fn dimensions(&self) -> ImageDimensions {
        self.view.image().dimensions()
    }
}

unsafe impl ImageViewAbstract for ImtImageView {
    #[inline]
    fn image(&self) -> Arc<dyn ImageAccess> {
        self.view.image().clone() as Arc<dyn ImageAccess>
    }

    #[inline]
    fn component_mapping(&self) -> ComponentMapping {
        self.view.component_mapping()
    }

    #[inline]
    fn filter_cubic(&self) -> bool {
        self.view.filter_cubic()
    }

    #[inline]
    fn filter_cubic_minmax(&self) -> bool {
        self.view.filter_cubic_minmax()
    }

    #[inline]
    fn format(&self) -> Option<Format> {
        self.view.format()
    }

    #[inline]
    fn format_features(&self) -> &FormatFeatures {
        self.view.format_features()
    }

    #[inline]
    fn sampler_ycbcr_conversion(&self) -> Option<&Arc<SamplerYcbcrConversion>> {
        self.view.sampler_ycbcr_conversion()
    }

    #[inline]
    fn subresource_range(&self) -> &ImageSubresourceRange {
        self.view.subresource_range()
    }

    #[inline]
    fn usage(&self) -> &ImageUsage {
        self.view.usage()
    }

    #[inline]
    fn view_type(&self) -> ImageViewType {
        self.view.view_type()
    }
}

unsafe impl VulkanObject for ImtImageView {
    type Object = ash::vk::ImageView;

    #[inline]
    fn internal_object(&self) -> ash::vk::ImageView {
        self.view.internal_object()
    }
}

unsafe impl DeviceOwned for ImtImageView {
    fn device(&self) -> &Arc<Device> {
        self.view.device()
    }
}

impl std::fmt::Debug for ImtImageView {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        write!(fmt, "<Vulkan image view {:?}>", self.internal_object())
    }
}

unsafe impl ImageAccess for ImtImageView {
    #[inline]
    fn inner(&self) -> ImageInner<'_> {
        self.view.image().inner()
    }

    #[inline]
    fn initial_layout_requirement(&self) -> ImageLayout {
        self.view.image().initial_layout_requirement()
    }

    #[inline]
    fn final_layout_requirement(&self) -> ImageLayout {
        self.view.image().final_layout_requirement()
    }

    #[inline]
    fn descriptor_layouts(&self) -> Option<ImageDescriptorLayouts> {
        self.view.image().descriptor_layouts()
    }
}
